use std::ffi::CStr;
use std::ptr;

use libc::c_char;
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_image::errors::ImageErrors;

use crate::enums::{ZImageColorspace, ZImageDepth, ZImageFormat};
use crate::errno::ZStatusType::{ZilDecodeErrors, ZilImageIsNull, ZilIoErrors};
use crate::errno::{ZStatus, ZStatusType};
use crate::utils::zil_free;
use crate::ZImage;

/// Get the image width from the image
#[no_mangle]
extern "C" fn zil_zimg_width(image: *mut ZImage, status: *mut ZStatus) -> usize {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return 0;
    }
    unsafe { (*image).dimensions().0 }
}

/// Get image height from image
#[no_mangle]
extern "C" fn zil_zimg_height(image: *mut ZImage, status: *mut ZStatus) -> usize {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return 0;
    }
    unsafe { (*image).dimensions().1 }
}

/// Get image depth from image
#[no_mangle]
extern "C" fn zil_zimg_depth(image: *mut ZImage, status: *mut ZStatus) -> ZImageDepth {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return ZImageDepth::ZilUnknownDepth;
    }
    unsafe { ZImageDepth::from((*image).depth()) }
}

/// Get image colorspace from image
#[no_mangle]
extern "C" fn zil_zimg_colorspace(image: *mut ZImage, status: *mut ZStatus) -> ZImageColorspace {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return ZImageColorspace::ZilUnknownColorspace;
    }
    unsafe { ZImageColorspace::from((*image).colorspace()) }
}

/// Get output size, this returns the minimum array needed to hold a single
/// interleaved frame of an image
///
/// \param image: A non-null image instance
/// \param status: A non-null status instance
///
/// \returns the number of bytes needed to store the image or 0 in case image is null
#[no_mangle]
extern "C" fn zil_zimg_get_out_buffer_size(image: *mut ZImage, status: *mut ZStatus) -> usize {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return 0;
    }
    let image = unsafe { &*image };

    let (w, h) = image.dimensions();
    let colorspace = image.colorspace().num_components();
    let depth = image.depth().size_of();

    w * h * colorspace * depth
}

/// Write image bytes to output array of output size
///
/// This writes interleaved raw pixels to buffer and returns bytes written
///
/// This is the preffered method to extract raw bytes from the image and can work with any
/// image type, provided you alias it here, i.e if you say for example want float hdr data
///
/// convert pointer to u8 and multiply output size by sizeof float
#[no_mangle]
pub extern "C" fn zil_zimg_write_to_output(
    image: *const ZImage, output: *mut u8, output_size: usize, status: *mut ZStatus
) -> usize {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return 0;
    }

    let output_array = unsafe { std::slice::from_raw_parts_mut(output, output_size) };

    let image = unsafe { &*image };
    let image_colorspace = image.colorspace();
    let channels = image.frames_ref()[0].channels_ref(image_colorspace, false);

    let result = match image.depth() {
        BitDepth::Eight => zune_image::utils::swizzle_channels(channels, output_array),
        BitDepth::Sixteen => {
            let (a, b, c) = unsafe { output_array.align_to_mut::<u16>() };

            if !a.is_empty() || !c.is_empty() {
                Err(ImageErrors::GenericStr("Unaligned output"))
            } else {
                zune_image::utils::swizzle_channels(channels, b)
            }
        }
        BitDepth::Float32 => {
            let (a, b, c) = unsafe { output_array.align_to_mut::<f32>() };

            if !a.is_empty() || !c.is_empty() {
                Err(ImageErrors::GenericStr("Unaligned output"))
            } else {
                zune_image::utils::swizzle_channels(channels, b)
            }
        }
        _ => Err(ImageErrors::GenericStr("Unknown depth"))
    };
    match result {
        Ok(bytes) => bytes * image.depth().size_of(),
        Err(err) => {
            if !status.is_null() {
                unsafe {
                    *status = ZStatus::new(err.to_string(), ZStatusType::ZilImageOperationError)
                }
            }
            0
        }
    }
}

/// Create an empty dummy image struct
///
/// This can now be passed to functions that require a pointer to a ZImage
///
/// This is the preferred way to initialize this, not via memset or malloc+sizeof
#[no_mangle]
pub extern "C" fn zil_zimg_new() -> *mut ZImage {
    let ptr = unsafe { libc::malloc(std::mem::size_of::<ZImage>()) }.cast();
    unsafe { *ptr = ZImage::new(vec![], BitDepth::Unknown, 1, 1, ColorSpace::Unknown) };
    ptr
}
/// Free an image
///
/// This drops the image and associated memory buffers
#[no_mangle]
pub extern "C" fn zil_zimg_free(image: *mut ZImage) {
    if !image.is_null() {
        // free object
        unsafe { image.drop_in_place() }

        // free memory holding it
        unsafe { zil_free(image.cast()) }
    }
}
/// Open an image from disk into memory
///
/// This is the method recommended to use when you want to decode an image and apply operations
/// to it
///
/// \param file: A nul-terminated file containing an image
/// \param image: After decoding, this will point to the image
/// \param status: Status information
#[no_mangle]
pub extern "C" fn zil_zimg_open(file: *const c_char, image: *mut ZImage, status: *mut ZStatus) {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return;
    }
    let c_str = unsafe { CStr::from_ptr(file) }.to_bytes();

    match std::str::from_utf8(c_str) {
        Ok(bytes) => match ZImage::open(bytes) {
            Ok(im) => {
                unsafe { *image = im };
            }
            Err(err) => {
                if !status.is_null() {
                    unsafe { *status = ZStatus::new(err.to_string(), ZilDecodeErrors) };
                }
            }
        },
        Err(err) => {
            if !status.is_null() {
                unsafe { *status = ZStatus::new(err.to_string(), ZilIoErrors) };
            }
        }
    }
}

/// Decode an image already in memory
///
///
/// \param input: Input memory containing encoded image bytes
/// \param input_size: The size of input
/// \param image: After decoding, this will point to the image
/// \param status: Status information
#[no_mangle]
pub extern "C" fn zil_zimg_read_from_memory(
    input: *const u8, input_size: usize, image: *mut ZImage, status: *mut ZStatus
) {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return;
    }
    let input_array = unsafe { std::slice::from_raw_parts(input, input_size) };
    let buffer = ZCursor::new(input_array);

    match ZImage::read(buffer, DecoderOptions::new_fast()) {
        Ok(im) => {
            unsafe { *image = im };
        }
        Err(err) => {
            if !status.is_null() {
                unsafe { *status = ZStatus::new(err.to_string(), ZilDecodeErrors) };
            }
        }
    };
}

/// \brief Write an image to disk
///
/// Format is inferred from the file extension
///
/// \param output_file: Output file to write into
///
/// \param image: The image we will be writing
///
/// \param status: Image operation status, query this to know if the operation succeeded
#[no_mangle]
pub extern "C" fn zil_zimg_write_to_disk(
    output_file: *const c_char, image: *const ZImage, status: *mut ZStatus
) {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return;
    }

    let c_str = unsafe { CStr::from_ptr(output_file) }.to_bytes();

    let image = unsafe { &*image };
    match std::str::from_utf8(c_str) {
        Ok(bytes) => {
            if let Err(e) = image.save(bytes) {
                if !status.is_null() {
                    unsafe { *status = ZStatus::new(e.to_string(), ZilDecodeErrors) };
                }
            }
        }
        Err(err) => {
            if !status.is_null() {
                unsafe { *status = ZStatus::new(err.to_string(), ZilIoErrors) };
            }
        }
    }
}

/// \brief Write an image to disk
///
/// Format is the one passed to the function
///
/// \param output_file: Output file to write into
///
/// \param image: The image we will be writing
///
/// \param format: The image format to use for encoding
///
/// \param status: Image operation status, query this to know if the operation succeeded
#[no_mangle]
pub extern "C" fn zil_zimg_write_to_disk_with_format(
    output_file: *const c_char, image: *const ZImage, format: ZImageFormat, status: *mut ZStatus
) {
    if image.is_null() {
        if !status.is_null() {
            unsafe { *status = ZStatus::new("Image null", ZilImageIsNull) };
        }
        return;
    }

    let c_str = unsafe { CStr::from_ptr(output_file) }.to_bytes();

    let image = unsafe { &*image };
    match std::str::from_utf8(c_str) {
        Ok(bytes) => {
            if let Err(e) = image.save_to(bytes, format.to_format()) {
                if !status.is_null() {
                    unsafe { *status = ZStatus::new(e.to_string(), ZilDecodeErrors) };
                }
            }
        }
        Err(err) => {
            if !status.is_null() {
                unsafe { *status = ZStatus::new(err.to_string(), ZilIoErrors) };
            }
        }
    }
}

/// Create a new copy of the image independent from the previous
/// one and return it
///
/// \param image: The image to clone
/// \returns: A fresh new copy of the image if everything goes well, otherwise null to indicate faliure
#[no_mangle]
extern "C" fn zil_zimg_clone(image: *const ZImage) -> *mut ZImage {
    if image.is_null() {
        return ptr::null_mut();
    }
    let image = unsafe { &*image };
    let new_img = zil_zimg_new();

    if new_img.is_null() {
        return ptr::null_mut();
    }
    unsafe { *new_img = image.clone() };
    new_img
}

/// Create an image from u8 pixels, the depth will be a bit depth of eight bits per pixel
///
/// The pixels are expected to be interleaved format, so if image is in
/// RGB, pixels should be in R,G,B,R,G,B
///
/// \param pixels: Pointer to first pixel
/// \param length: Length of the pixel array
/// \param width: The image width
/// \param height: The image height
/// \param colorspace: The image colorspace
///
#[no_mangle]
pub extern "C" fn zil_zimg_from_u8(
    pixels: *const u8, length: usize, width: usize, height: usize, colorspace: ZImageColorspace
) -> *mut ZImage {
    let pixels = unsafe { std::slice::from_raw_parts(pixels, length) };

    if let Some(size) = checked_mul(
        width,
        height,
        1,
        colorspace.to_colorspace().num_components()
    ) {
        if size <= length {
            let pix = &pixels[..size];
            let img = zil_zimg_new();
            if img.is_null() {
                return ptr::null_mut();
            }
            unsafe {
                *img = ZImage::from_u8(pix, width, height, colorspace.to_colorspace());
            }
            return img;
        }
    }
    ptr::null_mut()
}

/// Create an image from u16 pixels, the depth will be a bit depth of 16 bits per pixel
///
///
/// \param pixels: Pointer to first pixel
/// \param length: Length of the pixel array
/// \param width: The image width
/// \param height: The image height
/// \param colorspace: The image colorspace
///
#[no_mangle]
pub extern "C" fn zil_zimg_from_u16(
    pixels: *const u16, length: usize, width: usize, height: usize, colorspace: ZImageColorspace
) -> *mut ZImage {
    let pixels = unsafe { std::slice::from_raw_parts(pixels, length) };

    if let Some(size) = checked_mul(
        width,
        height,
        2,
        colorspace.to_colorspace().num_components()
    ) {
        if size <= length {
            let pix = &pixels[..size];
            let img = zil_zimg_new();
            if img.is_null() {
                return ptr::null_mut();
            }
            unsafe {
                *img = ZImage::from_u16(pix, width, height, colorspace.to_colorspace());
            }
            return img;
        }
    }
    ptr::null_mut()
}

/// Create an image from f32 pixels, the depth will be a bit depth of 32 bits per pixel
///
///
/// \param pixels: Pointer to first pixel
/// \param length: Length of the pixel array
/// \param width: The image width
/// \param height: The image height
/// \param colorspace: The image colorspace
///
#[no_mangle]
pub extern "C" fn zil_zimg_from_f32(
    pixels: *const f32, length: usize, width: usize, height: usize, colorspace: ZImageColorspace
) -> *mut ZImage {
    let pixels = unsafe { std::slice::from_raw_parts(pixels, length) };

    if let Some(size) = checked_mul(
        width,
        height,
        4,
        colorspace.to_colorspace().num_components()
    ) {
        if size <= length {
            let pix = &pixels[..size];
            let img = zil_zimg_new();
            if img.is_null() {
                return ptr::null_mut();
            }
            unsafe {
                *img = ZImage::from_f32(pix, width, height, colorspace.to_colorspace());
            }
            return img;
        }
    }
    ptr::null_mut()
}
fn checked_mul(
    width: usize, height: usize, depth: usize, colorspace_components: usize
) -> Option<usize> {
    width
        .checked_mul(height)?
        .checked_mul(depth)?
        .checked_mul(colorspace_components)
}
