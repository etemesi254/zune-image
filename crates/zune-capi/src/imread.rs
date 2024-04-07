use std::ffi::{c_char, CStr};
use std::ptr;

use zune_core::bit_depth::{BitDepth, ByteEndian};
use zune_core::bytestream::{ZByteReaderTrait, ZCursor, ZReader};
use zune_core::result::DecodingResult;
use zune_image::codecs::bmp::BmpDecoder;
use zune_image::codecs::farbfeld::FarbFeldDecoder;
use zune_image::codecs::hdr::HdrDecoder;
use zune_image::codecs::jpeg::JpegDecoder;
use zune_image::codecs::png::PngDecoder;
use zune_image::codecs::ppm::PPMDecoder;
use zune_image::codecs::psd::PSDDecoder;
use zune_image::codecs::ImageFormat;
use zune_image::errors::ImageErrors;

use crate::enums::{ZImageColorspace, ZImageDepth, ZImageFormat};
use crate::errno::{zil_status_ok, ZStatus, ZStatusType};
use crate::structs::ZImageMetadata;
use crate::utils::zil_malloc;

/// Read image contents of a file and return a pointer to the decoded bytes
///
///
/// The allocator used is `libc::malloc`
///
/// @param file: The file to decode
///
/// @param width: Image width, after successful decoding the value stored will be the image width,
/// can be null
///
/// @param height: Image height, after successful decoding, the value stored will be the image height,can be null
///
/// @param depth: Image depth, after successful decoding, the value stored will be the image depth,can be null
///
/// @param channels: Number of channels in the image, after successful decoding, the value stored will be the
/// image channels, can be null
///
/// @param status: Image decoding status, query this before inspecting contents of buf, CANNOT be null
///
/// \returns A pointer to the first element of the image pixels, the length of this array is strictly
/// `width * height * channels * depth`
///
/// In case the decoder cannot decode, returns `null` and the reason
/// why it can't be decoded is given in `status` parameter
///
#[no_mangle]
pub extern "C" fn zil_imread(
    file: *const c_char, width: *mut usize, height: *mut usize, depth: *mut ZImageDepth,
    channels: *mut usize, status: *mut ZStatus
) -> *const c_char {
    // safety: The caller is supposed to uphold this
    let binding = unsafe { CStr::from_ptr(file) }.to_string_lossy();
    let file_cstr = binding.as_ref();

    match std::fs::read(file_cstr) {
        Ok(data) => {
            if let Some(im_metadata) = zune_image::utils::decode_info(ZCursor::new(&data)) {
                // allocate a space big enough
                let (w, h) = im_metadata.dimensions();
                let colorspace = im_metadata.colorspace().num_components();
                let im_depth = im_metadata.depth().size_of();

                let new_size = w * h * colorspace * im_depth;

                let output = unsafe { zil_malloc(new_size) };
                if output.is_null() {
                    unsafe {
                        *status = ZStatus::new(
                            format!("Malloc failed to allocate buffer with size of {}", new_size),
                            ZStatusType::ZilMallocFailed
                        )
                    };
                    return ptr::null();
                }
                zil_imdecode_into(
                    data.as_ptr(),
                    data.len(),
                    output.cast(),
                    new_size,
                    width,
                    height,
                    depth,
                    channels,
                    status
                );
                if zil_status_ok(status) {
                    return output.cast();
                }
            } else {
                unsafe {
                    *status = ZStatus::new("ZilUnknownDepth image format", ZStatusType::ZilGeneric)
                };
                return ptr::null();
            }
        }
        Err(e) => {
            unsafe { *status = ZStatus::new(e.to_string(), ZStatusType::ZilIoErrors) };
            return ptr::null();
        }
    }
    ptr::null()
}

///\brief Load an image from a file and return it's contents
///
/// if `status` parameter  is null, returns early
///
///
/// @param file: File path, MUST be null terminated
///
/// @param buf: Output buffer, contents of decoding will be written here
///
/// @param buf_size: Buffer size, the size of output buffer
///
/// @param width: Image width, after successful decoding the value stored will be the image width,
/// can be null
///
/// @param height: Image height, after successful decoding, the value stored will be the image height,can be null
///
/// @param depth: Image depth, after successful decoding, the value stored will be the image depth,can be null
///
/// @param channels: Number of channels in the image, after successful decoding, the value stored will be the
/// image channels, can be null
///
/// @param status: Image decoding status, query this before inspecting contents of buf, CANNOT be null
#[no_mangle]
pub extern "C" fn zil_imread_into(
    file: *const c_char, output: *mut u8, output_size: usize, width: *mut usize,
    height: *mut usize, depth: *mut ZImageDepth, channels: *mut usize, status: *mut ZStatus
) {
    if status.is_null() {
        return;
    }
    // safety: The caller is supposed to uphold this
    let binding = unsafe { CStr::from_ptr(file) }.to_string_lossy();
    let file_cstr = binding.as_ref();

    match std::fs::read(file_cstr) {
        Ok(contents) => zil_imdecode_into(
            contents.as_ptr(),
            contents.len(),
            output,
            output_size,
            width,
            height,
            depth,
            channels,
            status
        ),
        Err(err) => {
            unsafe { *status = ZStatus::new(err.to_string(), ZStatusType::ZilIoErrors) };
        }
    }
}

/// Read image headers from a file and return common information such as width, height depth and colorspace
///
/// \param file: Null terminated
#[no_mangle]
pub extern "C" fn zil_read_headers_from_file(
    file: *const c_char, status: *mut ZStatus
) -> ZImageMetadata {
    if status.is_null() {
        return ZImageMetadata::default();
    }
    // safety: The caller is supposed to uphold this
    let binding = unsafe { CStr::from_ptr(file) }.to_string_lossy();
    let file_cstr = binding.as_ref();

    match std::fs::read(file_cstr) {
        Ok(bytes) => zil_read_headers_from_memory(bytes.as_ptr(), bytes.len() as _, status),
        Err(error) => {
            unsafe {
                (*status) = ZStatus::new(error.to_string(), ZStatusType::ZilIoErrors);
            };
            ZImageMetadata::default()
        }
    }
}

/// \brief  Decode image headers  of bytes already in memory
///
/// This reads and returns common image metadata, like width, depth,colorspace
/// it does not attempt to return extra details of images such as exif
///
/// \returns: A struct containing details and sets status to be successful In case of failure in decoding or status being null, returns a zeroed struct.
///
#[no_mangle]
pub extern "C" fn zil_read_headers_from_memory(
    input: *const u8, input_size: usize, status: *mut ZStatus
) -> ZImageMetadata {
    if status.is_null() {
        return ZImageMetadata::default();
    }
    unsafe {
        (*status) = ZStatus::new(
            "Could not decode headers, unknown error",
            ZStatusType::ZilDecodeErrors
        );
    };
    let contents = unsafe { std::slice::from_raw_parts(input, input_size) };

    match zune_image::utils::decode_info(ZCursor::new(contents)) {
        None => ZImageMetadata::default(),
        Some(metadata) => {
            let (w, h) = metadata.dimensions();

            unsafe { (*status) = ZStatus::okay() };

            ZImageMetadata {
                width:      w as u32,
                height:     h as u32,
                depth:      ZImageDepth::from(metadata.depth()),
                colorspace: ZImageColorspace::from(metadata.colorspace()),
                format:     ZImageFormat::from(
                    metadata.image_format().unwrap_or(ImageFormat::Unknown)
                )
            }
        }
    }
}
///\brief Decode an image already in memory
///
/// This decodes an image loaded to memory, and returns a pointer to the first pixel
///
/// The size of the array is strictly `image_width * image_height * image_depth * channels`
///
/// @param input: Input array of image bytes
/// @param input_size: Input size for the image bytes
/// @param width: Image width, will be filled after decoding with the decoded image width, can be null
/// @param height: Image height, will be filled after decoding with the decoded image height, can be null
/// @param depth: Image depth, will be filled after decoding with the decoded image depth, can be null
/// @param channels: Number of channels, will be filled after decoding with the decoded image channels, can be null
/// @param status: Image status,used to inform the caller if operations were successful
///
#[no_mangle]
pub extern "C" fn zil_imdecode(
    input: *const u8, input_size: usize, width: *mut usize, height: *mut usize,
    depth: *mut ZImageDepth, channels: *mut usize, status: *mut ZStatus
) -> *const c_char {
    if status.is_null() {
        return ptr::null();
    }
    let contents = unsafe { std::slice::from_raw_parts(input, input_size) };

    match zune_image::utils::decode_info(ZCursor::new(contents)) {
        None => {
            let msg = "Could not decode headers".to_string();
            // safety: We checked above if status is null
            unsafe { *status = ZStatus::new(msg, ZStatusType::ZilDecodeErrors) };
            return ptr::null();
        }
        Some(metadata) => {
            let (w, h) = metadata.dimensions();
            let im_depth = metadata.depth();
            let colorspace = metadata.colorspace();
            let size = w * h * im_depth.size_of() * colorspace.num_components();

            let output = unsafe { zil_malloc(size) };
            if output.is_null() {
                unsafe {
                    *status = ZStatus::new(
                        format!("Malloc failed to allocate buffer with size of {}", size),
                        ZStatusType::ZilMallocFailed
                    )
                };
                return ptr::null();
            }
            zil_imdecode_into(
                input,
                input_size,
                output.cast(),
                size,
                width,
                height,
                depth,
                channels,
                status
            );

            if zil_status_ok(status) {
                return output.cast();
            }
        }
    }
    ptr::null()
}

/// Decode from a byte array in memory and write pixels to `output`
///
/// Pixels written are strictly `image_width * image_height * image_depth * channels`
///
/// @param input: Input array of image bytes
/// @param input_size: Input size for the image bytes
/// @param output: Output array where to write decoded pixels
/// @param output_size: Size of `output`
/// @param width: Image width, will be filled after decoding with the decoded image width, can be null
/// @param height: Image height, will be filled after decoding with the decoded image height, can be null
/// @param depth: Image depth, will be filled after decoding with the decoded image depth, can be null
/// @param channels: Number of channels, will be filled after decoding with the decoded image channels, can be null
/// @param status: Image status,used to inform the caller if operations were successful
///
#[no_mangle]
pub extern "C" fn zil_imdecode_into(
    input: *const u8, input_size: usize, output: *mut u8, output_size: usize, width: *mut usize,
    height: *mut usize, depth: *mut ZImageDepth, channels: *mut usize, status: *mut ZStatus
) {
    if status.is_null() {
        return;
    }

    let contents = unsafe { std::slice::from_raw_parts(input, input_size) };
    // Safety the caller is supposed to uphold this
    let buf = unsafe { std::slice::from_raw_parts_mut(output, output_size) };

    match zune_image::utils::decode_info(ZCursor::new(contents)) {
        None => {
            let msg = "Could not decode headers".to_string();
            // safety: We checked above if status is null
            unsafe { *status = ZStatus::new(msg, ZStatusType::ZilDecodeErrors) };
        }
        Some(metadata) => {
            let (w, h) = metadata.dimensions();
            let im_depth = metadata.depth();
            let colorspace = metadata.colorspace();
            let size = w * h * im_depth.size_of() * colorspace.num_components();

            // the buffer has to be that big
            if buf.len() < size {
                let msg = format!("Expected buffer of size {},but found {}", size, buf.len());
                // safety, we checked above if status is null
                unsafe { *status = ZStatus::new(msg, ZStatusType::ZilNotEnoughSpaceInDest) };
                return;
            }

            if let Err(e) = imdecode_inner(ZCursor::new(contents), buf) {
                unsafe { *status = ZStatus::new(e.to_string(), ZStatusType::ZilDecodeErrors) };
                return;
            }
            // write parameters
            if !width.is_null() {
                unsafe { *width = w };
            }
            if !height.is_null() {
                unsafe { *height = h };
            }
            if !depth.is_null() {
                unsafe { *depth = ZImageDepth::from(im_depth) };
            }
            if !channels.is_null() {
                unsafe { *channels = colorspace.num_components() };
            }

            // safety, we checked above if the status is null
            unsafe { (*status).status = ZStatusType::ZilOk }
        }
    }
}

fn imdecode_inner<T>(data: T, output: &mut [u8]) -> Result<(), ImageErrors>
where
    T: ZByteReaderTrait
{
    if let Some((im_format, data)) = zune_image::codecs::guess_format(data) {
        match im_format {
            ImageFormat::JPEG => {
                // just write into buffer
                let mut decoder = JpegDecoder::new(data);

                decoder.decode_into(output)?;
            }
            ImageFormat::PNG => {
                // note: PNG has 8 bit and 16 bit images, it's a common format so we have to do some optimizations
                //
                // we don't strip 16 bit to 8 bit automatically, so we need to  handle that path
                // but we have `decode_into` only taking &[u8] slices, and making it generic and sucks
                //
                // so we branch on the depth, cheat a bit on 16 bit and return whatever we can
                // we expect the caller to have appropriately taken care of allocating enough to hold 16 bit
                //
                let mut decoder = PngDecoder::new(data);

                match decoder.depth().unwrap() {
                    BitDepth::Eight => {
                        decoder.decode_into(output)?;

                        return Ok(());
                    }
                    BitDepth::Sixteen => {
                        // safety:
                        // we can alias strong types to weak types, e.g u16->u8 works, we only care
                        // about alignment so it should be fine
                        //
                        // Reason:
                        // Saves us an unnecessary image allocation which is expensive
                        // set sample endianness to match platform
                        #[cfg(target_endian = "little")]
                        {
                            let options = decoder.options().set_byte_endian(ByteEndian::LE);
                            decoder.set_options(options);
                        }
                        #[cfg(target_endian = "big")]
                        {
                            let options = decoder.get_options().set_byte_endian(ByteEndian::BE);
                            decoder.set_options(options);
                        }

                        decoder.decode_into(output)?;
                    }
                    _ => unreachable!()
                }
            }
            ImageFormat::PPM => {
                let mut decoder = PPMDecoder::new(data);
                decoder.decode_headers()?;
                let (w, h) = decoder.dimensions().unwrap();
                let color = decoder.colorspace().unwrap();
                let depth = decoder.bit_depth().unwrap().size_of();
                let size = w * h * color.num_components() * depth;

                if output.len() < size {
                    return Err(ImageErrors::GenericString(format!(
                        "Too small of output buffer, expected {} but found {} ",
                        size,
                        output.len()
                    )));
                }

                match decoder.decode()? {
                    DecodingResult::U8(bytes) => output[..size].copy_from_slice(&bytes[..]),
                    DecodingResult::U16(bytes) => {
                        // alias u16 to u8
                        // SAFETY: u8 can alias everything
                        let (_, b, _) = unsafe { bytes.align_to::<u8>() };
                        output[..size].copy_from_slice(&b[..size]);
                    }
                    DecodingResult::F32(bytes) => {
                        // alias u16 to u8
                        // SAFETY: u8 can alias everything
                        let (_, b, _) = unsafe { bytes.align_to::<u8>() };
                        output[..size].copy_from_slice(&b[..size]);
                    }
                    _ => unreachable!()
                }
            }
            ImageFormat::PSD => {
                let mut decoder = PSDDecoder::new(data);
                decoder.decode_headers()?;
                let (w, h) = decoder.dimensions().unwrap();
                let color = decoder.colorspace().unwrap();
                let depth = decoder.bit_depth().unwrap().size_of();
                let size = w * h * color.num_components() * depth;

                if output.len() < size {
                    return Err(ImageErrors::GenericString(format!(
                        "Too small of output buffer, expected {} but found {} ",
                        size,
                        output.len()
                    )));
                }

                match decoder.decode()? {
                    DecodingResult::U8(bytes) => output[..size].copy_from_slice(&bytes[..]),
                    DecodingResult::U16(bytes) => {
                        // alias u16 to u8
                        // SAFETY: u8 can alias everything
                        let (_, b, _) = unsafe { bytes.align_to::<u8>() };
                        output[..size].copy_from_slice(&b[..size]);
                    }
                    DecodingResult::F32(bytes) => {
                        // alias u16 to u8
                        // SAFETY: u8 can alias everything
                        let (_, b, _) = unsafe { bytes.align_to::<u8>() };
                        output[..size].copy_from_slice(&b[..size]);
                    }
                    _ => unreachable!()
                }
            }
            ImageFormat::Farbfeld => {
                let mut decoder = FarbFeldDecoder::new(data);

                let (a, output_buf, c) = unsafe { output.align_to_mut() };

                if !a.is_empty() || !c.is_empty() {
                    // misalignment
                    return Err(ImageErrors::GenericStr("Buffer misalignment"));
                }
                decoder
                    .decode_into(output_buf)
                    .map_err(|x| ImageErrors::GenericString(format!("{:?}", x)))?;
            }
            ImageFormat::QOI => {
                // just write into buffer
                let mut decoder = JpegDecoder::new(data);

                decoder.decode_into(output)?;
            }
            ImageFormat::JPEG_XL => {
                let decoder = zune_image::codecs::jpeg_xl::jxl_oxide::JxlImage::builder()
                    .read(ZReader::new(data))
                    .map_err(|x| ImageErrors::GenericString(x.to_string()))?;

                let render = decoder
                    .render_frame(0)
                    .map_err(|x| ImageErrors::GenericString(x.to_string()))?;

                let (a, f32_buf, c) = unsafe { output.align_to_mut() };

                if !(a.is_empty() && c.is_empty()) {
                    // misalignment
                    return Err(ImageErrors::GenericStr("Buffer misalignment"));
                }

                let im_plannar = render.image();
                let buf_len = im_plannar.buf().len();

                if buf_len > f32_buf.len() {
                    return Err(ImageErrors::GenericStr(
                        "Too small of a buffer for jxl output"
                    ));
                }
                f32_buf[..buf_len].copy_from_slice(im_plannar.buf())
            }
            ImageFormat::HDR => {
                let mut decoder = HdrDecoder::new(data);

                let (a, f32_buf, c) = unsafe { output.align_to_mut() };

                if !(a.is_empty() && c.is_empty()) {
                    // misalignment
                    return Err(ImageErrors::GenericStr("Buffer misalignment"));
                }
                decoder.decode_into(f32_buf)?
            }
            ImageFormat::BMP => {
                let mut decoder = BmpDecoder::new(data);

                decoder.decode_into(output)?
            }
            _ => {}
        }
    }
    Ok(())
}
