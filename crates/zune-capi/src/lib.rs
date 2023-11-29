//! C bindings to zune-image
use zune_image::image::Image;

mod enums;
mod errno;
mod imread;
mod structs;
mod utils;

type ZImage = Image;
///
#[no_mangle]
pub extern "C" fn __chkstk() {}

#[no_mangle]
pub extern "C" fn _fltused() {}

#[test]
fn hello() {
    use crate::enums::ZImageDepth;
    use crate::errno::{zil_status_new, zil_status_ok};

    use std::ffi::{CStr, CString};

    let file = r"C:\Users\eteme\OneDrive\Pictures\Backgrounds\ameen-fahmy-mXpTl4jNKiA-unsplash.jpg";
    let c_str = CString::new(file).unwrap();
    let mut w = 0;
    let mut h = 0;
    let mut depth = ZImageDepth::UnknownDepth;
    let mut channels = 0;
    let mut status = zil_status_new();

    let c = imread::zil_imread(
        c_str.as_ptr(),
        &mut w,
        &mut h,
        &mut depth,
        &mut channels,
        &mut status,
    );

    let d = 0;
    if !zil_status_ok(&status) {
        panic!("{:?}", unsafe { CStr::from_ptr(status.message) });
    }
    println!("{},{},{}", w, h, channels);
}
