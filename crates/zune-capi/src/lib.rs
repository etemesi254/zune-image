//! C bindings to zune-image

use zune_image::image::Image;

mod enums;
mod errno;
mod image;
mod improc;
mod imread;
mod structs;
mod utils;

pub type ZImage = Image;
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn __chkstk() {}

#[cfg(target_os = "windows")]
#[no_mangle]
pub extern "C" fn _fltused() {}

#[test]
fn hello() {
    // use crate::utils::zil_free;

    //use crate::errno::{zil_status_free, zil_status_message};

    // use crate::enums::ZImageDepth;
    //use crate::errno::{zil_status_new, zil_status_ok};

    //use std::ffi::CStr;
    //use std::ffi::CString;

    // // let mut file = env!("CARGO_MANIFEST_DIR").to_string();
    // // let c = PathBuf::from("/test-images/basn0g01.png");
    // //     c.
    // // println!("{:?}", file);
    // let c_str = CString::new(file).unwrap();
    // let mut w = 0;
    // let mut h = 0;
    // let mut depth = ZImageDepth::ZilUnknownDepth;
    // let mut channels = 0;
    // let mut status = zil_status_new();
    //
    // let ptr = imread::zil_imread(
    //     c_str.as_ptr(),
    //     &mut w,
    //     &mut h,
    //     &mut depth,
    //     &mut channels,
    //     &mut status,
    // );
    //
    // if !zil_status_ok(&status) {
    //     panic!("{:?}", unsafe {
    //         CStr::from_ptr(zil_status_message(&status))
    //     });
    // }
    // zil_status_free(status);
    // unsafe { zil_free(ptr as _) };
    // println!("{},{},{}", w, h, channels);
}

#[test]
fn test_status_works() {
    use crate::errno::zil_status_new;
    use crate::utils::zil_free;

    unsafe {
        let c = zil_status_new();
        assert!(!c.is_null());
        zil_free(c.cast());
    }
}
