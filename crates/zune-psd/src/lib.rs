//! A simple PSD decoder
//!
//! This crate features a simple Photoshop PSD reader
//!
//! ## What it means by simple
//! Photoshop is a complicated format, probably one of the most complicated format,
//! this library doesn't claim to parse a lot of the images photoshop  and it's derivatives will generate.
//!
//! It does not check layers, doesn't like CMYKa images, only reads Grayscale, RGB and RGBA images
//! ignoring Dutone, Multichannel and a slew of other `.PSD` features I've never heard of.
//! It's as simple as it gets.
//!
//! Sometimes that's all you need..
//!
//! [obligatory photoshop dunking](https://github.com/gco/xee/blob/master/XeePhotoshopLoader.m#L108)
//!
//! # Example
//! - Reading a psd file
//! ```no_run
//! use zune_psd::errors::PSDDecodeErrors;
//! use zune_core::result::DecodingResult;
//! use zune_psd::PSDDecoder;
//!
//! fn main()->Result<(),PSDDecodeErrors>{
//!     use zune_core::bytestream::ZCursor;
//! let mut decoder = PSDDecoder::new(ZCursor::new(&[]));
//!     let px = decoder.decode()?;
//!
//!     // we need to handle u8 and u16 since the decoder supports those depths
//!     match px {
//!         DecodingResult::U8(_) => {}
//!         DecodingResult::U16(_) => {}
//!         _=>unreachable!()
//!     };
//!     Ok(())
//! }
//! ```
//!
//#![forbid(unsafe_code)]
#![no_std]
extern crate alloc;
pub extern crate zune_core;
pub use decoder::PSDDecoder;

mod constants;
pub mod decoder;
pub mod errors;
