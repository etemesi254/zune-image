use std::ffi::c_void;
use std::ptr::null_mut;

use libc::size_t;
use zune_core::bytestream::ZCursor;

use crate::enums::ZImageFormat;

/// \brief Guess the format of an image
///
/// This function inspects the first few bytes of an image file
/// to determine the actual image codec the file is in
///
/// If the format cannot be deduced or it's unknown returns `ZImageFormat::ZilUnknownDepth`
///
/// @param bytes: A memory address containing image data, must point to a valid memory address
/// @param size: Size of the bytes parameter, must not exceed `bytes` length
///
/// @returns ZImageFormat the image format of the bytes, or ZImageFormat::ZilUnknownDepth if the image is unknown
///
#[no_mangle]
pub unsafe extern "C" fn zil_guess_format(bytes: *const u8, size: usize) -> ZImageFormat {
    let slice = std::slice::from_raw_parts(bytes, size);

    match zune_image::codecs::guess_format(ZCursor::new(slice)) {
        None => ZImageFormat::ZilUnknownFormat,
        Some((format, _)) => ZImageFormat::from(format)
    }
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
    assert!(!ptr.is_null(), "Trying to free a null ptr!!");
    if !ptr.is_null() {
        libc::free(ptr);
        // set it to be null
    }
}

/// Free a memory region that was allocated by zil_malloc or internally by the and set it to null
///
/// E.g. free a pointer returned by `zil_imread`
///
/// \param ptr: A pointer allocated by `zil_malloc`
#[no_mangle]
pub unsafe extern "C" fn zil_free_and_null(ptr: *mut *mut c_void) {
    if !ptr.is_null() {
        let deref_ptr = *ptr;
        zil_free(deref_ptr);
        *ptr = null_mut()
    }
}
