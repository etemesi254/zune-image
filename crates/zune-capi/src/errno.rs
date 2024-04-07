use std::ffi::{c_char, CString};
use std::mem::size_of;
use std::ptr;

use crate::utils::{zil_free, zil_malloc};

/// Various representations of things that may go wrong
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum ZStatusType {
    /// Everything is okay, operation succeeded
    ZilOk = 0,
    /// The buffer passed to a function wasn't enough to
    /// store the results
    ZilNotEnoughSpaceInDest,
    /// An error that doesn't fit into a specific genre
    ZilGeneric,
    /// An error originating from decoding
    ZilDecodeErrors,
    /// An error originating from Input output errors
    ZilIoErrors,
    /// Malloc failed
    ZilMallocFailed,
    /// Status is null, indicates the passed status value is null
    /// useful when we have been asked for status code but
    /// passed a null status
    ZilNullStatus,
    /// Image is null
    ///
    /// An operation expecting a non_null image got a null image
    ZilImageIsNull,
    /// Image operation failed
    ZilImageOperationError // Image encoding failed
                           //ImageEncodingFailed
}

/// A status indicator that tells you more about things that went wrong
///
///
/// To create an instance use `zil_status_new`
///
/// To get an enum which contains more details about the execution use `zil_status_code`
/// and to get the message raised by an exception use `zil_status_message`
///
/// For quickly checking if an operation succeeded, you can use `zil_status_ok` that
/// returns a boolean indicating whether something worked, true if operation succeeded, false otherwise
///
/// To free the structure use
///
#[repr(C)]
pub struct ZStatus {
    pub status:  ZStatusType,
    /// A short message indicating what went wrong
    pub message: *mut char
}

impl ZStatus {
    pub fn new<T>(message: T, status: ZStatusType) -> ZStatus
    where
        T: Into<Vec<u8>>
    {
        let msg = CString::new(message).unwrap();
        let mem = unsafe { zil_malloc(msg.as_bytes_with_nul().len()) };
        // copy to memory
        unsafe {
            libc::strcpy(mem.cast(), msg.as_ptr());
        }

        ZStatus {
            status,
            message: mem.cast()
        }
    }
    /// Return okay
    pub fn okay() -> ZStatus {
        ZStatus::new("Ok", ZStatusType::ZilOk)
    }
}

impl Drop for ZStatus {
    fn drop(&mut self) {
        unsafe { zil_free(self.message.cast()) };
    }
}
/// \brief Check if image operation succeeded
///
/// @param status: Image status
///
/// @returns true if everything is okay, if status is null or something went bad returns false
#[no_mangle]
pub extern "C" fn zil_status_ok(status: *const ZStatus) -> bool {
    if status.is_null() {
        return false;
    }
    unsafe { (*status).status == ZStatusType::ZilOk }
}

/// Create a new image status struct
///
/// This can be passed around to functions that report progress via
/// status
///
/// Remember to free it with `zil_status_free`
#[no_mangle]
pub unsafe extern "C" fn zil_status_new() -> *mut ZStatus {
    let ptr = zil_malloc(size_of::<ZStatus>());

    let msg = CString::new("").unwrap();
    let mem = unsafe { zil_malloc(msg.as_bytes_with_nul().len()) };
    if mem.is_null() {
        return ptr::null_mut();
    }
    // copy to memory
    unsafe {
        libc::strcpy(mem.cast(), msg.as_ptr());
    }
    if !ptr.is_null() {
        (*ptr.cast::<ZStatus>()).message = mem.cast();
        (*ptr.cast::<ZStatus>()).status = ZStatusType::ZilOk;
    }
    // make pointer
    ptr.cast()
}

/// Return the status code contained in the ZImStatus
///
/// \param status The status struct for which to extract a status from
///
/// \returns ZStatusCode, an enum that indicates if everything is okay or something went wrong
#[no_mangle]
pub extern "C" fn zil_status_code(status: *const ZStatus) -> ZStatusType {
    if status.is_null() {
        return ZStatusType::ZilNullStatus;
    }
    // safety, checked above if it's null
    unsafe { (*status).status }
}

/// Returns a null terminated string that contains more details about
/// what went wrong
///
/// \param status: The image status for which we are extracting the message from
///
/// \returns: The message contained, if `status` is null, returns null
#[no_mangle]
pub extern "C" fn zil_status_message(status: *const ZStatus) -> *const c_char {
    if status.is_null() {
        return ptr::null();
    }
    unsafe { (*status).message.cast() }
}

/// Destroy a status indicator.
///
/// This takes by value and drops the status param
/// freeing any memory allocated and used by this status struct
///
/// \param status: The status to free
#[no_mangle]
pub extern "C" fn zil_status_free(status: *mut ZStatus) {
    if !status.is_null() {
        unsafe { zil_free((*status).message.cast()) }
        // free object
        unsafe { status.drop_in_place() }

        // free memory holding it
        unsafe { zil_free(status.cast()) }
    }
}
