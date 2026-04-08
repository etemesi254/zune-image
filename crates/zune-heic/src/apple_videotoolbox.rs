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
//! 6. Frames are converted from NV12 (YUV) → RGB and stored in a shared `TileMap`.
//!
//! ## Threading Model
//!
//! - Decoding is asynchronous.
//! - The callback is invoked on a VideoToolbox-managed background thread.
//! - Output is stored in a `Arc<Mutex<HashMap<u32, Vec<u8>>>>` (`TileMap`).
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
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
use crate::decoder::{HeifDecoder, SingleDecodedTile, TileMap};
use crate::errors::HeicErrors;
use crate::processor::HevcSample;

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
    /// Typically, `context` is a pointer to a `TileMap`.
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
/// VideoToolbox decode callback.
///
/// This function is invoked asynchronously when a frame is decoded.
///
/// # Responsibilities
///
/// - Extract NV12 planes (Y + interleaved UV)
/// - Convert to RGB
/// - Store in `TileMap` using `item_id`
///
/// # Safety
///
/// - `decompression_output_ref_con` must point to a valid `Mutex<HashMap<...>>`.
/// - `source_frame_ref_con` must be a valid encoded `item_id`.
extern "C" fn decode_callback(
    decompression_output_ref_con: *mut c_void, source_frame_ref_con: *mut c_void, status: OSStatus,
    _info_flags: VTDecodeInfoFlags, image_buffer: CVImageBufferRef,
    _presentation_time_stamp: CMTime, _presentation_duration: CMTime
) {
    let tile_map_ptr = decompression_output_ref_con
        as *const Mutex<HashMap<u32, Result<SingleDecodedTile, HeicErrors>>>;
    let item_id = source_frame_ref_con as usize;

    if status != 0 || image_buffer.is_null() {
        let msg = format!("Hardware decode failed. Status: {status}");
        unsafe {
            if let Ok(mut map) = (*tile_map_ptr).lock() {
                map.insert(item_id as u32, Err(HeicErrors::Generic { msg }));
            }
            return;
        }
    }

    unsafe {
        CVPixelBufferLockBaseAddress(image_buffer, 1);

        let width = CVPixelBufferGetWidth(image_buffer);
        let height = CVPixelBufferGetHeight(image_buffer);

        let y_ptr = CVPixelBufferGetBaseAddressOfPlane(image_buffer, 0).cast_const();
        let y_stride = CVPixelBufferGetBytesPerRowOfPlane(image_buffer, 0);
        let uv_ptr = CVPixelBufferGetBaseAddressOfPlane(image_buffer, 1).cast_const();
        let uv_stride = CVPixelBufferGetBytesPerRowOfPlane(image_buffer, 1);

        let y_plane = slice::from_raw_parts(y_ptr, height * y_stride);
        let uv_plane = slice::from_raw_parts(uv_ptr, (height / 2) * uv_stride);

        let mut rgb_data = vec![0u8; width * height * 3];
        let mut out_pos = 0usize;

        let chunks_of_16 = width / 16;
        let remainder = width % 16;

        let mut cb_chunk = [0i16; 16];
        let mut cr_chunk = [0i16; 16];

        for row in 0..height {
            let y_row_base = row * y_stride;
            let uv_row_base = (row / 2) * uv_stride;

            for chunk in 0..chunks_of_16 {
                let x_base = chunk * 16;

                // Y: one contiguous slice read
                let y_src = &y_plane[y_row_base + x_base..][..16];
                let y_chunk: [i16; 16] = std::array::from_fn(|i| i16::from(y_src[i]));

                // UV: one 16-byte slice read, deinterleave into cb/cr in 8 iterations
                let uv_base = uv_row_base + (x_base / 2) * 2;
                let uv_src = &uv_plane[uv_base..][..16];

                for i in 0..8 {
                    let cb = i16::from(uv_src[i * 2]);
                    let cr = i16::from(uv_src[i * 2 + 1]);
                    cb_chunk[i * 2] = cb;
                    cb_chunk[i * 2 + 1] = cb;
                    cr_chunk[i * 2] = cr;
                    cr_chunk[i * 2 + 1] = cr;
                }

                ycbcr_to_rgb_inner_16_scalar::<false>(
                    &y_chunk,
                    &cb_chunk,
                    &cr_chunk,
                    &mut rgb_data,
                    &mut out_pos
                );
            }

            // Remainder: same idea but clamp to avoid OOB
            if remainder > 0 {
                let x_base = chunks_of_16 * 16;

                let y_chunk: [i16; 16] = std::array::from_fn(|i| {
                    i16::from(y_plane[y_row_base + (x_base + i).min(width - 1)])
                });

                let mut cb_chunk = [0i16; 16];
                let mut cr_chunk = [0i16; 16];
                for i in 0..8 {
                    let x0 = (x_base + i * 2).min(width - 1);
                    let x1 = (x_base + i * 2 + 1).min(width - 1);
                    // x0 is always even after the min clamp, but guard with & !1
                    let uv_off0 = uv_row_base + (x0 & !1);
                    let uv_off1 = uv_row_base + (x1 & !1);
                    let cb0 = i16::from(uv_plane[uv_off0]);
                    let cr0 = i16::from(uv_plane[uv_off0 + 1]);
                    let cb1 = i16::from(uv_plane[uv_off1]);
                    let cr1 = i16::from(uv_plane[uv_off1 + 1]);
                    cb_chunk[i * 2] = cb0;
                    cb_chunk[i * 2 + 1] = cb1;
                    cr_chunk[i * 2] = cr0;
                    cr_chunk[i * 2 + 1] = cr1;
                }

                let mut temp = [0u8; 48];
                let mut temp_pos = 0usize;
                ycbcr_to_rgb_inner_16_scalar::<false>(
                    &y_chunk,
                    &cb_chunk,
                    &cr_chunk,
                    &mut temp,
                    &mut temp_pos
                );

                let valid_bytes = remainder * 3;
                rgb_data[out_pos..out_pos + valid_bytes].copy_from_slice(&temp[..valid_bytes]);
                out_pos += valid_bytes;
            }
        }

        CVPixelBufferUnlockBaseAddress(image_buffer, 1);

        if let Ok(mut map) = (*tile_map_ptr).lock() {
            let tile = SingleDecodedTile {
                width:  width,
                height: height,
                pixels: rgb_data
            };
            map.insert(item_id as u32, Ok(tile));
        }
    }
}

impl<T: ZByteReaderTrait> HeifDecoder<T> {
    /// Decode HEVC tiles using Apple VideoToolbox hardware acceleration.
    ///
    /// # Returns
    ///
    /// A `TileMap` containing all decoded tiles indexed by `item_id`.
    ///
    /// # Workflow
    ///
    /// 1. Lazily initializes `AppleHardwareDecoder`
    /// 2. Feeds HEVC samples into VideoToolbox
    /// 3. Waits for all frames via `flush()`
    /// 4. Returns collected RGB tiles
    ///
    /// # Notes
    ///
    /// - Decoding is asynchronous internally.
    /// - Final `flush()` is required to guarantee completion.
    pub(crate) fn decode_hardware_videotoolbox(&mut self) -> Result<TileMap, HeicErrors> {
        // 1. Thread-safe tile storage
        let tile_map: TileMap = Arc::new(Mutex::new(HashMap::new()));
        let mut hardware_decoder: Option<AppleHardwareDecoder> = None;

        // 2. The Loop
        let mut processor = |sample: HevcSample| -> Result<(), HeicErrors> {
            if hardware_decoder.is_none() {
                // Initialize with a pointer to our tile_map
                let vps = sample.vps.as_deref().ok_or(HeicErrors::Generic {
                    msg: "vps not found".to_owned()
                })?;
                let sps = sample.sps.as_deref().ok_or(HeicErrors::Generic {
                    msg: "sps not found".to_owned()
                })?;
                let pps = sample.pps.as_deref().ok_or(HeicErrors::Generic {
                    msg: "pps not found".to_owned()
                })?;

                // Pass the RAW POINTER of the mutex to the callback
                let context_ptr = Arc::as_ptr(&tile_map) as *mut c_void;
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
                decoder.decode_sample(&sample).unwrap();
            }
            Ok(())
        };

        self.process_hevc_samples(&mut processor)?;

        if let Some(decoder) = hardware_decoder.as_ref() {
            decoder.flush(); // Wait for all 48 tiles to hit the TileMap
        }

        Ok(tile_map)
    }
}
