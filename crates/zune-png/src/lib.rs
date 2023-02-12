//!A png decoder
//!
//! # Usage
//!
//! #### Decode to raw bytes.
//!
//! This is a simple decode operation which returns raw
//! bytes of the image.
//!
//! - **Note**: The interpretation of the data varies depending
//! on the endianness of the source image, for 16 bit depth images
//! each two bytes represent a single pixel in native endian.
//! So one should inspect `PngDecoder::get_bit_depth` to get bit depth
//! of image in order to understand the raw bytes layout.
//!
//! A more convenient API is given below, using `decode`
//!
//! ```no_run
//! use zune_png::PngDecoder;
//! let mut decoder = PngDecoder::new(&[]);
//!
//! let pixels = decoder.decode_raw();
//! ```
//!
//! # Decode to u8 or u16 depending on depth
//!
//! From above limitation, there are needs to treat result
//! types differently depending on the image's bit depth.
//!
//! That's what the `decode` api for the PngDecoder does.
//!
//!```no_run
//! use zune_png::PngDecoder;
//! use zune_core::result::DecodingResult;
//! let mut decoder = PngDecoder::new(&[]);
//!
//! let pixels = decoder.decode().unwrap();
//!
//! match pixels {
//!    DecodingResult::U8(_px)=>{
//!        // do something with images with 8 bit depths
//!    }
//!    DecodingResult::U16(_px)=>{
//!        // do something with images with 16 bit depths
//!    }
//!}
//!```
//! The above has a more complicated API, but it ensures that you
//! handle any image depth correctly.
//!
//! E.g one can make it that 16 bit images are scaled to 8 bit images.
//!
#![cfg_attr(feature = "std", no_std)]
extern crate alloc;

pub use decoder::PngDecoder;

mod constants;
mod crc;
mod decoder;
mod enums;
pub mod error;
mod filters;
mod headers;
mod options;
