//! A png decoder
//!
//! This features a simple PNG reader in Rust which supports decoding of valid
//! ISO/IEC 15948:2003 (E) or PNG images
//!
//!
//! # Features
//! - Fast inflate decoder
//! - Platform specific intrinsics for accelerated decoding on x86
//! - Endian aware decoding support.
//!
//! # Usage
//! Add the library to `Cargo.toml`
//!
//! ```toml
//! zune_png="0.2"
//! ```
//!
//! #### Decode to raw bytes.
//!
//! This is a simple decode operation which returns raw
//! bytes of the image.
//!
//! - **Note**: The interpretation of the data varies depending
//! on the endianness of the source image, for 16 bit depth images
//! each two bytes represent a single pixel in a configurable endian.
//! So one should inspect `PngDecoder::get_bit_depth` to get bit depth
//! of image in order to understand the raw bytes layout.
//!
//! A more convenient API is given below, using `decode`
//!
//!```no_run
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
//!    DecodingResult::U8(px)=>{
//!        // do something with images with 8 bit depths
//!    }
//!    DecodingResult::U16(px)=>{
//!        // do something with images with 16 bit depths
//!    }
//!    _=>unreachable!(),
//!}
//!```
//! The above has a more complicated API, but it ensures that you
//! handle any image depth correctly.
//!
//! E.g one can make it that 16 bit images are scaled to 8 bit images.
//!
//! # Endian aware decoding support
//!
//! One can set the target endianness of bits for 16 bit images by using
//! [`DecoderOptions::set_endian`](zune_core::options::DecoderOptions::set_byte_endian) which
//! will be respected by [`decode_raw`](decoder::PngDecoder::decode_raw) and [`decode_into`](decoder::PngDecoder::decode_into) functions
//!
//!
//! # Extracting metadata
//!
//! Once headers have been decoded, image metadata can be accessed via [`get_info()`](PngDecoder::get_info) method
//!
//! Some data is usually borrowed from the underlying reader, so the lifetime of the [`PngInfo`] struct is tied
//! to the lifetime of the [`PngDecoder`] struct from which it was derived
//!
//! # Alternatives
//! - [png](https://crates.io/crates/png) crate
//!
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::op_ref, clippy::identity_op)]
extern crate alloc;

pub use decoder::{ItxtChunk, PngDecoder, PngInfo, TextChunk, TimeInfo, ZtxtChunk};
pub use enums::InterlaceMethod;
pub use zune_core;

mod constants;
mod crc;
mod decoder;
mod enums;
pub mod error;
mod filters;
mod headers;
mod options;
mod utils;
