//! Entry point for all supported codecs  
//! the library contains
//!
//! Current status
//!
//! |IMAGE    | Decoder      |Encoder|
//! |---------|--------------|-------|
//! |JPEG     |Full support  | None |
//! |PNG      |Partial       |None |
//! |PPM      | 8 and 16 bit support |8 and 16 bit support|
//! |PAL      | None |8 and 16 bit support |
//! | Farbfeld|16 bit support|None|
//!
//!
#![allow(unused_imports, unused_variables)]

use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use crate::codecs;
use crate::image_format::ImageFormat;
use crate::traits::{DecoderTrait, EncoderTrait};

pub mod farbfeld;
pub mod jpeg;
pub mod png;
pub mod ppm;
pub mod psd;
pub mod qoi;
