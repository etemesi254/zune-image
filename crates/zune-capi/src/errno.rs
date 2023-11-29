use std::cell::RefCell;
use std::ffi::{c_char, CString};
use std::ptr;

/// Various representations of things that may go wrong
#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ZStatusType {
    /// Everything is okay, operation succeeded
    Ok = 0,
    /// The buffer passed to a function wasn't enough to
    /// store the results
    NotEnoughSpaceInDest,
    /// An error that doesn't fit into a specific genre
    Generic,
    /// An error originating from decoding
    DecodeErrors,
    /// An error originating from Input output errors
    IoErrors,
    /// Malloc failed
    MallocFailed,
    /// Status is null, indicates the passed status value is null
    /// useful when we have been asked for status code but
    /// passed a null status
    NullStatus,
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
///
#[repr(C)]
pub struct ZStatus {
    pub status: ZStatusType,
}
thread_local! {
    // we need a place to store image information that outlives the caller.
    //
    // we use this for that
    //
    // We cannot pass CString to C, since it doesn't
    // know and understand it,and since everything is exposed in C, the struct is unknown to it
    static CSTRING_GLOBAL_STORAGE:RefCell<CString> = RefCell::new(CString::new("OK").unwrap());
}

impl ZStatus {
    pub fn new<T>(message: T, status: ZStatusType) -> ZStatus
    where
        T: Into<Vec<u8>>,
    {
        CSTRING_GLOBAL_STORAGE.with_borrow_mut(|x| *x = CString::new(message).unwrap());
        ZStatus { status }
    }
    /// Return okay
    pub fn okay() -> ZStatus {
        ZStatus::new("Ok", ZStatusType::Ok)
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
    unsafe { (*status).status == ZStatusType::Ok }
}

/// Create a new image status struct
///
/// This can be passed around to functions that report progress via
/// status
#[no_mangle]
pub extern "C" fn zil_status_new() -> ZStatus {
    ZStatus::new("", ZStatusType::Ok)
}

/// Return the status code contained in the ZImStatus
///
/// \param status The status struct for which to extract a status from
///
/// \returns ZStatusCode, an enum that indicates if everything is okay or something went wrong
#[no_mangle]
pub extern "C" fn zil_status_code(status: *const ZStatus) -> ZStatusType {
    if status.is_null() {
        return ZStatusType::NullStatus;
    }
    // safety, checked above if it's null
    return unsafe { (*status).status };
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
    return CSTRING_GLOBAL_STORAGE.with_borrow(|x| x.as_ptr());
}
