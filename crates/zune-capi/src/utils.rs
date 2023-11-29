use crate::enums::ZImageFormat;
use libc::size_t;
use std::ffi::{c_long, c_uchar, c_void};

/// \brief Guess the format of an image
///
/// This function inspects the first few bytes of an image file
/// to determine the actual image codec the file is in
///
/// If the format cannot be deduced or it's unknown returns `ZImageFormat::UnknownDepth`
///
/// @param bytes: A memory address containing image data, must point to a valid memory address
/// @param size: Size of the bytes parameter, must not exceed `bytes` length
///
/// @returns ZImageFormat the image format of the bytes, or ZImageFormat::UnknownDepth if the image is unknown
///
#[no_mangle]
pub unsafe extern "C" fn zil_guess_format(bytes: *const c_uchar, size: c_long) -> ZImageFormat {
    let slice = std::slice::from_raw_parts(bytes, size as usize);

    return match zune_image::codecs::guess_format(slice) {
        None => ZImageFormat::UnknownFormat,
        Some((format, _)) => ZImageFormat::from(format),
    };
}
/// Allocate a region of memory
///
/// This uses libc's malloc hence on most platforms it should be the system allocator
///
/// \param size: Memory size
#[no_mangle]
pub unsafe extern "C" fn zil_malloc(size: size_t) -> *mut c_void {
    libc::malloc(size)
}

/// Free a memory region that was allocated by zil_malloc or internally by the library
///
/// E.g. free a pointer returned by `zil_imread`
///
/// \param ptr: A pointer allocated by `zil_malloc`
///
#[no_mangle]
pub unsafe extern "C" fn zil_free(ptr: *mut c_void) {
    libc::free(ptr)
}
