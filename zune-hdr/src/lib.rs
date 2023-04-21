//! A RADIANCE HDR decoder
//!
//!
//! # Features
//! - Minimal interface, few dependencies
//! - `no_std` by default with alloc feature
//! - Fast.
//!
//! # Usage notes
//! The decoders returns data in `&[f32]` types with the exponent already added to the numbers
//! it does not return raw data nor does it expose the ability to do so.
//!
//!
//! # Metadata
//! - Radiance images usually store metadata in key value pairs.
//! During decoding, we extract this metadata from the headers into a hashmap for which we provide
//! via get_metadata method, the decoder does not in any way interpret the metadata to understand
//! the image characteristics or colorspace, it is the caller's work to do that.
//!
//! Some important metadata include the image colorspace, which may be present or not,
//! color primaries, exposure,gamma e.t.c,
//!
#![no_std]
#![macro_use]
extern crate alloc;

pub use decoder::HdrDecoder;
pub use errors::HdrDecodeErrors;

mod decoder;
mod errors;
