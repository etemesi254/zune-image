#![cfg(target_os = "macos")]
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::{ptr, slice};

use core_foundation_sys::base::{CFAllocatorRef, CFRelease, OSStatus, kCFAllocatorNull};
use core_media_sys::{
    CMBlockBufferCreateWithMemoryBlock, CMBlockBufferRef, CMSampleBufferRef, CMTime,
    CMVideoFormatDescriptionRef
};
use video_toolbox_sys::cv_types::CVImageBufferRef;
use video_toolbox_sys::decompression::{
    VTDecodeInfoFlags, VTDecompressionOutputCallbackRecord, VTDecompressionSessionCreate,
    VTDecompressionSessionDecodeFrame, VTDecompressionSessionRef,
    VTDecompressionSessionWaitForAsynchronousFrames
};
use zune_core::bytestream::ZByteReaderTrait;

use crate::decoder::HeifDecoder;
use crate::errors::BmfErrors;
use crate::processor::HevcSample;
pub type TileMap = Arc<Mutex<HashMap<u32, Vec<u8>>>>;

unsafe extern "C" {
    /// Creates a CMVideoFormatDescription from HEVC (H.265) parameter-set NAL units.
    fn CMVideoFormatDescriptionCreateFromHEVCParameterSets(
        allocator: CFAllocatorRef,
        parameter_set_count: usize,
        parameter_set_pointers: *const *const u8,
        parameter_set_sizes: *const usize,
        nal_unit_header_length: i32,
        extensions: *const c_void, // CFDictionaryRef — pass NULL
        format_description_out: *mut CMVideoFormatDescriptionRef
    ) -> OSStatus;

    /// Creates a ready-to-use CMSampleBuffer wrapping an existing CMBlockBuffer.
    fn CMSampleBufferCreateReady(
        allocator: CFAllocatorRef,
        data_buffer: CMBlockBufferRef,
        format_description: CMVideoFormatDescriptionRef,
        num_samples: isize,
        num_sample_timing_entries: isize,
        sample_timing_array: *const c_void, // CMSampleTimingInfo* — pass NULL
        num_sample_size_entries: isize,
        sample_size_array: *const usize, // pass NULL
        sample_buffer_out: *mut CMSampleBufferRef
    ) -> OSStatus;

}
#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVPixelBufferLockBaseAddress(pixelBuffer: CVImageBufferRef, lockFlags: u32) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pixelBuffer: CVImageBufferRef, lockFlags: u32) -> i32;
    fn CVPixelBufferGetWidth(pixelBuffer: CVImageBufferRef) -> usize;
    fn CVPixelBufferGetHeight(pixelBuffer: CVImageBufferRef) -> usize;
    fn CVPixelBufferGetBaseAddressOfPlane(
        pixelBuffer: CVImageBufferRef, planeIndex: usize
    ) -> *mut u8;
    fn CVPixelBufferGetBytesPerRowOfPlane(
        pixelBuffer: CVImageBufferRef, planeIndex: usize
    ) -> usize;
}
// ---------------------------------------------------------------------------

pub struct AppleHardwareDecoder {
    session:     VTDecompressionSessionRef,
    format_desc: CMVideoFormatDescriptionRef
}

impl AppleHardwareDecoder {
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
                &mut format_desc
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
                &callback_record,
                &mut session
            );

            if status != 0 {
                CFRelease(format_desc as *mut c_void);
                return Err(status);
            }

            Ok(Self {
                session,
                format_desc
            })
        }
    }

    /// Block until all in-flight async frames have been delivered to the callback.
    pub fn flush(&self) {
        unsafe {
            VTDecompressionSessionWaitForAsynchronousFrames(self.session);
        }
    }

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
                    &mut block_buffer
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
                    &mut sample_buffer
                );
                if status != 0 {
                    CFRelease(block_buffer as *mut c_void);
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
                    1,            // Enable Asynchronous
                    frame_id_ptr, // <-- NEW: Pass the ID here!
                    ptr::null_mut()
                );

                // 4. Release our local references.
                //    VideoToolbox retains whatever it still needs internally.
                CFRelease(sample_buffer as *mut c_void);
                CFRelease(block_buffer as *mut c_void);

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
                CFRelease(self.session as *mut c_void);
            }
            if !self.format_desc.is_null() {
                CFRelease(self.format_desc as *mut c_void);
            }
        }
    }
}

extern "C" fn decode_callback(
    decompression_output_ref_con: *mut c_void,
    source_frame_ref_con: *mut c_void, // Removed underscore so we can use it!
    status: OSStatus,
    _info_flags: VTDecodeInfoFlags,
    image_buffer: CVImageBufferRef, // This is our typed pixel buffer
    _presentation_time_stamp: CMTime,
    _presentation_duration: CMTime
) {
    // 0 = kCVReturnSuccess
    if status != 0 || image_buffer.is_null() {
        eprintln!("Hardware decode failed. Status: {}", status);
        return;
    }
    let tile_map_ptr = decompression_output_ref_con as *const Mutex<HashMap<u32, Vec<u8>>>;

    // Recover our item_id from the C pointer (Passed during VTDecompressionSessionDecodeFrame)
    let item_id = source_frame_ref_con as usize;

    unsafe {
        // 1. Lock the hardware memory (1 = kCVPixelBufferLock_ReadOnly)
        CVPixelBufferLockBaseAddress(image_buffer, 1);

        // CoreVideo returns dimensions as usize
        let width = CVPixelBufferGetWidth(image_buffer);
        let height = CVPixelBufferGetHeight(image_buffer);

        // Plane 0: The Y (Luma/Grayscale) channel. Full resolution.
        let y_ptr = CVPixelBufferGetBaseAddressOfPlane(image_buffer, 0) as *const u8;
        let y_stride = CVPixelBufferGetBytesPerRowOfPlane(image_buffer, 0);

        // Plane 1: The UV (Chroma/Color) channel. Interleaved (U,V,U,V). Half resolution.
        let uv_ptr = CVPixelBufferGetBaseAddressOfPlane(image_buffer, 1) as *const u8;
        let uv_stride = CVPixelBufferGetBytesPerRowOfPlane(image_buffer, 1);

        // Create safe Rust slices over the Apple hardware memory
        let y_plane = slice::from_raw_parts(y_ptr, height * y_stride);
        let uv_plane = slice::from_raw_parts(uv_ptr, (height / 2) * uv_stride);

        // Allocate our RGB output buffer
        let mut rgb_data = vec![0u8; width * height * 3];
        let mut rgb_idx = 0;

        // 2. The YCbCr 4:2:0 to RGB Math Loop
        for y in 0..height {
            for x in 0..width {
                // Get Luma
                let y_val = y_plane[y * y_stride + x] as f32;

                // Get Chroma (Divided by 2 because UV is subsampled in 4:2:0)
                let uv_x = x / 2;
                let uv_y = y / 2;

                // UV plane is interleaved: U is at index 0, V is at index 1
                let uv_offset = (uv_y * uv_stride) + (uv_x * 2);
                let u_val = uv_plane[uv_offset] as f32 - 128.0;
                let v_val = uv_plane[uv_offset + 1] as f32 - 128.0;

                // Standard Full-Range BT.709 YCbCr to RGB Conversion
                let r = (y_val + 1.402 * v_val).clamp(0.0, 255.0) as u8;
                let g = (y_val - 0.344136 * u_val - 0.714136 * v_val).clamp(0.0, 255.0) as u8;
                let b = (y_val + 1.772 * u_val).clamp(0.0, 255.0) as u8;

                rgb_data[rgb_idx] = r;
                rgb_data[rgb_idx + 1] = g;
                rgb_data[rgb_idx + 2] = b;
                rgb_idx += 3;
            }
        }

        // 3. Unlock the memory immediately so Apple can reuse the buffer for the next tile
        CVPixelBufferUnlockBaseAddress(image_buffer, 1);
        // Store in memory
        if let Ok(mut map) = (*tile_map_ptr).lock() {
            map.insert(item_id as u32, rgb_data);
        }
    }
}

impl<T: ZByteReaderTrait> HeifDecoder<T> {
    pub(crate) fn decode_hardware_videotoolbox(&mut self) -> Result<TileMap, BmfErrors> {
        // 1. Thread-safe tile storage
        let tile_map: TileMap = Arc::new(Mutex::new(HashMap::new()));
        let mut hardware_decoder: Option<AppleHardwareDecoder> = None;

        // 2. The Loop
        let mut processor = |sample: HevcSample| -> Result<(), BmfErrors> {
            if hardware_decoder.is_none() {
                // Initialize with a pointer to our tile_map
                let vps = sample.vps.as_deref().unwrap();
                let sps = sample.sps.as_deref().unwrap();
                let pps = sample.pps.as_deref().unwrap();

                // Pass the RAW POINTER of the mutex to the callback
                let context_ptr = Arc::as_ptr(&tile_map) as *mut c_void;
                hardware_decoder =
                    Some(AppleHardwareDecoder::new(vps, sps, pps, context_ptr).unwrap());
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
