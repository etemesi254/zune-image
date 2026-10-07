#![cfg(target_os = "macos")]
//! # Apple Hardware HEVC Decoder (VideoToolbox)
//!
//! This module provides a hardware-accelerated HEVC (H.265) decoder using
//! Apple's VideoToolbox framework.
//!
//! ## Overview
//!
//! The decoding pipeline works as follows:
//!
//! 1. VPS/SPS/PPS parameter sets are used to create a `CMVideoFormatDescription`.
//! 2. A `VTDecompressionSession` is created (hardware-accelerated when available).
//! 3. Encoded HEVC samples are wrapped into `CMSampleBuffer`s.
//! 4. Samples are submitted asynchronously to VideoToolbox.
//! 5. Decoded frames are delivered via a C callback (`decode_callback`).
//! 6. Each decoded frame is converted from NV12 (YUV) → RGB straight into its
//!    place in the output canvas (only the visible part of the tile).
//!
//! ## Threading Model
//!
//! - Decoding is asynchronous.
//! - The callback is invoked on a VideoToolbox-managed background thread.
//! - The canvas has one lock per row (same as the software decoder), so tiles
//!   can be written concurrently; errors and finished tiles are recorded in a
//!   `Mutex<CallbackStatus>`.
//!
//! ## Safety
//!
//! This module uses extensive `unsafe` code due to FFI with CoreMedia and
//! VideoToolbox. Key invariants:
//!
//! - Input buffers must remain valid for the duration of decoding.
//! - `kCFAllocatorNull` is used to prevent CoreMedia from freeing Rust-owned memory.
//! - The callback receives raw pointers which must remain valid.
//!
//! ## Platform
//!
//! Only available on **macOS**.
mod types;
use core::ffi::c_void;
use core::{ptr, slice};
use alloc::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use zune_core::bytestream::ZByteReaderTrait;

use crate::apple_videotoolbox::types::{
    CFRelease, CMBlockBufferCreateWithMemoryBlock, CMBlockBufferRef, CMSampleBufferCreateReady,
    CMSampleBufferRef, CMTime, CMVideoFormatDescriptionCreateFromHEVCParameterSets,
    CMVideoFormatDescriptionRef, CVImageBufferRef, CVPixelBufferGetBaseAddressOfPlane,
    CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeight, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress, OSStatus, VTDecodeInfoFlags,
    VTDecompressionOutputCallbackRecord, VTDecompressionSessionCreate,
    VTDecompressionSessionDecodeFrame, VTDecompressionSessionRef,
    VTDecompressionSessionWaitForAsynchronousFrames, kCFAllocatorNull
};
use crate::decoder::HeifDecoder;
use crate::errors::HeicErrors;
use crate::processor::HevcSample;
use crate::utils::Lock;

// ---------------------------------------------------------------------------
/// Hardware HEVC decoder backed by VideoToolbox.
///
/// This struct owns:
/// - A `VTDecompressionSession`
/// - A `CMVideoFormatDescription`
///
/// It is responsible for submitting encoded samples and receiving decoded frames
/// via a callback.
pub struct AppleHardwareDecoder {
    session:     VTDecompressionSessionRef,
    format_desc: CMVideoFormatDescriptionRef
}

impl AppleHardwareDecoder {
    /// Create a new hardware decoder instance.
    ///
    /// # Arguments
    ///
    /// * `vps` - Video Parameter Set
    /// * `sps` - Sequence Parameter Set
    /// * `pps` - Picture Parameter Set
    /// * `context` - Opaque pointer passed to the decode callback
    ///
    /// `context` is a pointer to the `CallbackContext` the callback writes into.
    ///
    /// # Returns
    ///
    /// - `Ok(Self)` if initialization succeeds
    /// - `Err(OSStatus)` if VideoToolbox/CoreMedia fails
    ///
    /// # Safety
    ///
    /// - `context` must remain valid for the lifetime of the decoder.
    /// - Parameter sets must follow HEVC spec.
    pub fn new(vps: &[u8], sps: &[u8], pps: &[u8], context: *mut c_void) -> Result<Self, OSStatus> {
        unsafe {
            let mut format_desc: CMVideoFormatDescriptionRef = ptr::null_mut();

            let parameter_set_pointers = [vps.as_ptr(), sps.as_ptr(), pps.as_ptr()];
            let parameter_set_sizes = [vps.len(), sps.len(), pps.len()];

            // 1. Build the HEVC format description from VPS / SPS / PPS
            let status = CMVideoFormatDescriptionCreateFromHEVCParameterSets(
                ptr::null_mut(), // allocator  (NULL = default)
                3,               // parameterSetCount
                parameter_set_pointers.as_ptr(),
                parameter_set_sizes.as_ptr(),
                4,           // NAL unit header length (4 bytes in HVCC)
                ptr::null(), // extensions (NULL = none)
                &raw mut format_desc
            );

            if status != 0 {
                return Err(status);
            }

            // 2. Configure the output callback
            let callback_record = VTDecompressionOutputCallbackRecord {
                decompressionOutputCallback: decode_callback,
                decompressionOutputRefCon:   context
            };

            // 3. Create the decompression session
            let mut session: VTDecompressionSessionRef = ptr::null_mut();

            let status = VTDecompressionSessionCreate(
                ptr::null_mut(), // allocator
                format_desc,
                ptr::null_mut(), // decoder specification  (NULL = auto-select hardware)
                ptr::null_mut(), // destination image buffer attributes (NULL = native YUV)
                &raw const callback_record,
                &raw mut session
            );

            if status != 0 {
                CFRelease(format_desc.cast::<c_void>());
                return Err(status);
            }

            Ok(Self {
                session,
                format_desc
            })
        }
    }
    /// Wait for all queued frames to finish decoding.
    ///
    /// This blocks until all asynchronous decode operations complete
    /// and their callbacks have been invoked.
    pub fn flush(&self) {
        unsafe {
            VTDecompressionSessionWaitForAsynchronousFrames(self.session);
        }
    }

    /// Submit an HEVC sample for decoding.
    ///
    /// # Behavior
    ///
    /// - Each extent is wrapped in a `CMBlockBuffer` (zero-copy).
    /// - Then wrapped in a `CMSampleBuffer`.
    /// - Submitted asynchronously to VideoToolbox.
    ///
    /// The decoded result is delivered via `decode_callback`.
    ///
    /// # Errors
    ///
    /// Returns `OSStatus` if any CoreMedia or VideoToolbox call fails.
    pub fn decode_sample(&self, sample: &HevcSample<'_>) -> Result<(), OSStatus> {
        unsafe {
            for extent in &sample.extents {
                // 1. Wrap the zero-copy Rust slice in a CMBlockBuffer.
                //    kCFAllocatorNull tells CoreMedia NOT to free the memory —
                //    Rust owns it and will drop it when HevcSample goes away.
                let mut block_buffer: CMBlockBufferRef = ptr::null_mut();
                let status = CMBlockBufferCreateWithMemoryBlock(
                    ptr::null_mut(),
                    extent.as_ptr() as *mut c_void,
                    extent.len(),
                    kCFAllocatorNull,
                    ptr::null_mut(),
                    0,
                    extent.len(),
                    0,
                    &raw mut block_buffer
                );
                if status != 0 {
                    return Err(status);
                }

                // 2. Wrap the CMBlockBuffer in a CMSampleBuffer.
                let mut sample_buffer: CMSampleBufferRef = ptr::null_mut();
                let status = CMSampleBufferCreateReady(
                    ptr::null_mut(), // allocator
                    block_buffer,
                    self.format_desc,
                    1,           // numSamples
                    0,           // numSampleTimingEntries (0 = not provided)
                    ptr::null(), // sampleTimingArray
                    0,           // numSampleSizeEntries  (0 = not provided)
                    ptr::null(), // sampleSizeArray
                    &raw mut sample_buffer
                );
                if status != 0 {
                    CFRelease(block_buffer as _);
                    return Err(status);
                }

                // 3. Hand the frame to VideoToolbox.
                //    Flag 1 = kVTDecodeFrame_EnableAsynchronousDecompression.
                //    Results arrive via decode_callback on a VT background thread.

                // Cast our item_id (u32) to a void pointer so C can carry it for us
                let frame_id_ptr = sample.item_id as usize as *mut c_void;

                // 3. Send to VideoToolbox Hardware
                let status = VTDecompressionSessionDecodeFrame(
                    self.session,
                    sample_buffer,
                    1, // Enable Asynchronous
                    frame_id_ptr,
                    ptr::null_mut()
                );

                // 4. Release our local references.
                //    VideoToolbox retains whatever it still needs internally.
                CFRelease(sample_buffer.cast::<c_void>());
                CFRelease(block_buffer as _);

                if status != 0 {
                    return Err(status);
                }
            }
            Ok(())
        }
    }
}

impl Drop for AppleHardwareDecoder {
    fn drop(&mut self) {
        unsafe {
            if !self.session.is_null() {
                CFRelease(self.session as _);
            }
            if !self.format_desc.is_null() {
                CFRelease(self.format_desc.cast::<c_void>());
            }
        }
    }
}

pub(crate) const Y_CF: i16 = 16384;
pub(crate) const CR_CF: i16 = 22970;
pub(crate) const CB_CF: i16 = 29032;
pub(crate) const C_G_CR_COEF_1: i16 = -11700;
pub(crate) const C_G_CB_COEF_2: i16 = -5638;
pub(crate) const YUV_PREC: i16 = 14;
// Rounding const for YUV -> RGB conversion: floating equivalent 0.499(9).
pub(crate) const YUV_RND: i16 = (1 << (YUV_PREC - 1)) - 1;

fn clamp(a: i32) -> u8 {
    a.clamp(0, 255) as u8
}
/// Convert a batch of 16 YCbCr pixels to RGB.
///
/// This is a scalar fallback implementation used during pixel conversion.
///
/// # Parameters
///
/// - `BGRA`: If true, output is written as BGRA order instead of RGB.
/// - `y`, `cb`, `cr`: Input YUV components (16 pixels)
/// - `output`: Destination buffer
/// - `pos`: Current write offset (updated after writing)
///
/// # Panics
///
/// Panics if output buffer is too small.
pub fn ycbcr_to_rgb_inner_16_scalar<const BGRA: bool>(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], output: &mut [u8], pos: &mut usize
) {
    let (_, output_position) = output.split_at_mut(*pos);

    // Convert into a slice with 48 elements
    let opt: &mut [u8; 48] = output_position
        .get_mut(0..48)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    for ((&y, (cb, cr)), out) in y
        .iter()
        .zip(cb.iter().zip(cr.iter()))
        .zip(opt.chunks_exact_mut(3))
    {
        let cr = cr - 128;
        let cb = cb - 128;

        let y0 = i32::from(y) * i32::from(Y_CF) + i32::from(YUV_RND);

        let r = (y0 + i32::from(cr) * i32::from(CR_CF)) >> YUV_PREC;
        let g = (y0
            + i32::from(cr) * i32::from(C_G_CR_COEF_1)
            + i32::from(cb) * i32::from(C_G_CB_COEF_2))
            >> YUV_PREC;
        let b = (y0 + i32::from(cb) * i32::from(CB_CF)) >> YUV_PREC;

        if BGRA {
            out[0] = clamp(b);
            out[1] = clamp(g);
            out[2] = clamp(r);
        } else {
            out[0] = clamp(r);
            out[1] = clamp(g);
            out[2] = clamp(b);
        }
    }

    // Increment pos
    *pos += 48;
}
/// Where decoded tiles go: the output canvas (one lock per row) and the grid
/// position(s) of every tile.
struct CallbackContext<'a, 'b> {
    canvas_rows: &'a [Lock<&'b mut [u8]>],
    placements:  &'a BTreeMap<u32, Vec<usize>>,
    cols:        usize,
    canvas_w:    usize,
    canvas_h:    usize,
    channels:    usize,
    status:      Mutex<CallbackStatus>
}

#[derive(Default)]
struct CallbackStatus {
    /// first error reported by any callback
    error: Option<HeicErrors>,
    /// tiles written successfully
    done:  BTreeSet<u32>
}

impl CallbackContext<'_, '_> {
    fn status(&self) -> std::sync::MutexGuard<'_, CallbackStatus> {
        self.status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn fail(&self, error: HeicErrors) {
        self.status().error.get_or_insert(error);
    }

    /// Convert a decoded NV12 tile into every grid position it occupies.
    #[allow(clippy::too_many_arguments)]
    fn write_tile(
        &self, item_id: u32, width: usize, height: usize, y_plane: &[u8], y_stride: usize,
        uv_plane: &[u8], uv_stride: usize
    ) -> Result<(), HeicErrors> {
        if width == 0 || height == 0 || y_stride < width || uv_stride < width.div_ceil(2) * 2 {
            return Err(HeicErrors::Generic {
                msg: format!("Unexpected hardware frame layout {width}x{height}")
            });
        }
        let positions = self.placements.get(&item_id).ok_or(HeicErrors::Generic {
            msg: format!("Decoded tile {item_id} has no grid position")
        })?;

        for &index in positions {
            let base_x = (index % self.cols) * width;
            let base_y = (index / self.cols) * height;
            if base_x >= self.canvas_w || base_y >= self.canvas_h {
                // tile lies completely outside the visible canvas
                continue;
            }
            let vis_w = width.min(self.canvas_w - base_x);
            let vis_h = height.min(self.canvas_h - base_y);

            for row in 0..vis_h {
                let y_row = &y_plane[row * y_stride..row * y_stride + width];
                let uv_start = (row / 2) * uv_stride;
                let uv_row = &uv_plane[uv_start..uv_start + uv_stride];

                self.canvas_rows[base_y + row].with(|dst_row| {
                    let dst = &mut dst_row
                        [base_x * self.channels..(base_x + vis_w) * self.channels];
                    convert_nv12_row(y_row, uv_row, vis_w, dst, self.channels);
                });
            }
        }
        Ok(())
    }
}

/// Convert the first `vis_w` pixels of one NV12 row (`y_row` is the full
/// luma row, `uv_row` the interleaved Cb/Cr row) into `dst`
/// (`vis_w * channels` bytes; 1 = luma, 3 = RGB, 4 = RGB + opaque alpha).
fn convert_nv12_row(y_row: &[u8], uv_row: &[u8], vis_w: usize, dst: &mut [u8], channels: usize) {
    if channels == 1 {
        dst[..vis_w].copy_from_slice(&y_row[..vis_w]);
        return;
    }
    let width = y_row.len();
    let full_chunks = vis_w / 16;
    let mut cb_chunk = [0i16; 16];
    let mut cr_chunk = [0i16; 16];
    let mut temp = [0u8; 48];
    let mut out_pos = 0usize;

    for chunk in 0..vis_w.div_ceil(16) {
        let x_base = chunk * 16;
        let is_full = chunk < full_chunks;

        let y_chunk: [i16; 16] = core::array::from_fn(|i| {
            // clamp so the last partial chunk never reads past the row
            i16::from(y_row[(x_base + i).min(width - 1)])
        });
        for i in 0..16 {
            // chroma sample covering luma column x (4:2:0, interleaved Cb/Cr)
            let uv = (x_base + i).min(width - 1) & !1;
            cb_chunk[i] = i16::from(uv_row[uv]);
            cr_chunk[i] = i16::from(uv_row[uv + 1]);
        }

        let n_px = if is_full { 16 } else { vis_w - x_base };
        if channels == 3 && is_full {
            ycbcr_to_rgb_inner_16_scalar::<false>(&y_chunk, &cb_chunk, &cr_chunk, dst, &mut out_pos);
            continue;
        }
        let mut temp_pos = 0usize;
        ycbcr_to_rgb_inner_16_scalar::<false>(&y_chunk, &cb_chunk, &cr_chunk, &mut temp, &mut temp_pos);
        if channels == 3 {
            dst[out_pos..out_pos + n_px * 3].copy_from_slice(&temp[..n_px * 3]);
            out_pos += n_px * 3;
        } else {
            // RGBA: alpha is not decoded yet, emit opaque pixels
            for (px, rgb) in dst[out_pos..out_pos + n_px * 4]
                .chunks_exact_mut(4)
                .zip(temp.chunks_exact(3))
            {
                px[..3].copy_from_slice(rgb);
                px[3] = 255;
            }
            out_pos += n_px * 4;
        }
    }
}

/// VideoToolbox decode callback.
///
/// This function is invoked asynchronously when a frame is decoded. It
/// converts the NV12 frame straight into the output canvas.
///
/// # Safety
///
/// - `decompression_output_ref_con` must point to the `CallbackContext` given
///   to `AppleHardwareDecoder::new`, which outlives the session.
/// - `source_frame_ref_con` carries the tile's `item_id`.
extern "C" fn decode_callback(
    decompression_output_ref_con: *mut c_void, source_frame_ref_con: *mut c_void, status: OSStatus,
    _info_flags: VTDecodeInfoFlags, image_buffer: CVImageBufferRef,
    _presentation_time_stamp: CMTime, _presentation_duration: CMTime
) {
    // SAFETY: see function docs; the context is only accessed through `&`.
    let ctx = unsafe { &*(decompression_output_ref_con as *const CallbackContext) };
    let item_id = source_frame_ref_con as usize as u32;

    if status != 0 || image_buffer.is_null() {
        ctx.fail(HeicErrors::Generic {
            msg: format!("Hardware decode of tile {item_id} failed. Status: {status}")
        });
        return;
    }

    let result = unsafe {
        CVPixelBufferLockBaseAddress(image_buffer, 1);

        let width = CVPixelBufferGetWidth(image_buffer);
        let height = CVPixelBufferGetHeight(image_buffer);

        let y_ptr = CVPixelBufferGetBaseAddressOfPlane(image_buffer, 0).cast_const();
        let y_stride = CVPixelBufferGetBytesPerRowOfPlane(image_buffer, 0);
        let uv_ptr = CVPixelBufferGetBaseAddressOfPlane(image_buffer, 1).cast_const();
        let uv_stride = CVPixelBufferGetBytesPerRowOfPlane(image_buffer, 1);

        let result = if y_ptr.is_null() || uv_ptr.is_null() {
            Err(HeicErrors::Generic {
                msg: format!("Hardware frame for tile {item_id} has no pixel data")
            })
        } else {
            // NV12: full-height luma plane, half-height (rounded up) chroma plane
            let y_plane = slice::from_raw_parts(y_ptr, height * y_stride);
            let uv_plane = slice::from_raw_parts(uv_ptr, height.div_ceil(2) * uv_stride);
            ctx.write_tile(item_id, width, height, y_plane, y_stride, uv_plane, uv_stride)
        };

        CVPixelBufferUnlockBaseAddress(image_buffer, 1);
        result
    };

    match result {
        Ok(()) => {
            ctx.status().done.insert(item_id);
        }
        Err(e) => ctx.fail(e)
    }
}

impl<T: ZByteReaderTrait> HeifDecoder<T> {
    /// Decode HEVC tiles using Apple VideoToolbox hardware acceleration,
    /// writing each tile straight into `canvas_rows` (one entry per canvas
    /// row of `canvas_w * channels` bytes) at its grid position(s).
    ///
    /// Decoding is asynchronous; this waits for every submitted frame before
    /// returning, so no callback can run after the canvas is released.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn decode_hardware_videotoolbox(
        &self, canvas_rows: &[Lock<&mut [u8]>], placements: &BTreeMap<u32, Vec<usize>>,
        cols: usize, canvas_w: usize, canvas_h: usize, channels: usize
    ) -> Result<(), HeicErrors> {
        // Declared before the decoder so it is dropped after it.
        let context = CallbackContext {
            canvas_rows,
            placements,
            cols,
            canvas_w,
            canvas_h,
            channels,
            status: Mutex::new(CallbackStatus::default())
        };
        let mut hardware_decoder: Option<AppleHardwareDecoder> = None;

        let mut processor = |sample: HevcSample| -> Result<(), HeicErrors> {
            if hardware_decoder.is_none() {
                let vps = sample.vps.as_deref().ok_or(HeicErrors::Generic {
                    msg: "vps not found".to_owned()
                })?;
                let sps = sample.sps.as_deref().ok_or(HeicErrors::Generic {
                    msg: "sps not found".to_owned()
                })?;
                let pps = sample.pps.as_deref().ok_or(HeicErrors::Generic {
                    msg: "pps not found".to_owned()
                })?;

                // The callback receives a pointer to `context`
                let context_ptr = (&raw const context).cast_mut().cast::<c_void>();
                hardware_decoder = Some(
                    AppleHardwareDecoder::new(vps, sps, pps, context_ptr).map_err(|d| {
                        HeicErrors::Generic {
                            msg: format!(
                                "Error when initializing apple hardware decoder: os-status:{d}"
                            )
                        }
                    })?
                );
            }

            if let Some(decoder) = hardware_decoder.as_ref() {
                decoder.decode_sample(&sample).map_err(|d| HeicErrors::Generic {
                    msg: format!(
                        "Error submitting tile {} to apple hardware decoder: os-status:{d}",
                        sample.item_id
                    )
                })?;
            }
            Ok(())
        };

        let submitted = self.process_hevc_samples(&mut processor);

        // Always wait for in-flight frames, even on error: their callbacks
        // write into the canvas.
        if let Some(decoder) = hardware_decoder.as_ref() {
            decoder.flush();
        }
        drop(hardware_decoder);
        submitted?;

        let status = context
            .status
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(error) = status.error {
            return Err(error);
        }
        if let Some(missing) = placements.keys().find(|id| !status.done.contains(id)) {
            return Err(HeicErrors::Generic {
                msg: format!("Tile missing from hardware decoder: {missing}")
            });
        }
        Ok(())
    }
}
