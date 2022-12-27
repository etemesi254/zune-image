//! An incredibly spiffy deflate decoder.
//!
//! This crate features a deflate/zlib decoder inspired by
//! Eric Bigger's [libdeflate] hence.
//!
//! This libary has a smaller set of features hence you should use it
//! if it aligns with your end goals.
//!
//! Use it if
//! - You want a smaller library footprint when compared to flate/miniz-oxide
//! - You want zlib-ng/libdeflate speeds
//! - You want a 100% safe, pure rust implementation with above.
//!
//!
//!
//! # Usage
//!
//! Decoding delfate data
//
//! ```no_run
//! use zune_inflate::DeflateDecoder;
//! let totally_valid_data = [0;23];
//! let mut decoder = DeflateDecoder::new(&totally_valid_data);
//!
//! let decompressed =decoder.decode_deflate();
//! ```
//!
//! Decoding zlib data
//! ```no_run
//! use zune_inflate::DeflateDecoder;
//! let totally_valid_data = [0;23];
//! let mut decoder = DeflateDecoder::new(&totally_valid_data);
//!
//! let decompressed =decoder.decode_zlib();
//! ```
//!
//! Decoding zlib data without confirming the adler32 checksum
//! ```no_run
//! use zune_inflate::DeflateDecoder;
//! use zune_inflate::DeflateOptions;
//! let totally_valid_data=[0;23];
//! let mut options = DeflateOptions::default()
//!                     .set_confirm_checksum(false);
//! let decoder =  DeflateDecoder::new_with_options(&totally_valid_data,options);
//!
//! ```
//!
//! [libdeflate]: https://github.com/ebiggers/libdeflate
pub use crate::decoder::{DeflateDecoder, DeflateOptions};

mod bitstream;
mod constants;
mod decoder;
pub mod errors;
mod utils;
