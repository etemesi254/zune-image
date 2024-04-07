use libc::c_uint;

use crate::enums::{ZImageColorspace, ZImageDepth, ZImageFormat};

/// Image metadata details
#[repr(C)]
pub struct ZImageMetadata {
    pub width:      c_uint,
    pub height:     c_uint,
    pub depth:      ZImageDepth,
    pub colorspace: ZImageColorspace,
    pub format:     ZImageFormat
}

impl Default for ZImageMetadata {
    fn default() -> Self {
        ZImageMetadata {
            width:      0,
            height:     0,
            depth:      ZImageDepth::ZilUnknownDepth,
            colorspace: ZImageColorspace::ZilUnknownColorspace,
            format:     ZImageFormat::ZilUnknownFormat
        }
    }
}
