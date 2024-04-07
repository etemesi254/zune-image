/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! A RADIANCE HDR decoder and encoder
//!
//!
//! # Features
//! - Minimal interface, few dependencies
//! - Fast.
//! - No unsafe
//! - Fuzz tested decoder
//!
//! # Usage notes
//! The decoder returns data in `&[f32]` types with the exponent already added to the numbers
//! it does not return raw data nor does it expose the ability to do so.
//!
//!
//! # Metadata
//! - Radiance images usually store metadata in key value pairs.
//!
//! During decoding, we extract this metadata from the headers into a hashmap for which we provide
//! via get_metadata method, the decoder does not in any way interpret the metadata to understand
//! the image characteristics or colorspace, it is the caller's work to do that.
//!
//! Some important metadata include the image colorspace, which may be present or not,
//! color primaries, exposure,gamma e.t.c,
//!

// CAE: No std doesn't work because we haven't implemented
// floor and exp2 for floats, which do not exist in no std land
// #![no_std]
#![forbid(unsafe_code)]
#![macro_use]
extern crate alloc;
extern crate core;
pub extern crate zune_core;
pub use decoder::HdrDecoder;
pub use encoder::HdrEncoder;
pub use errors::{HdrDecodeErrors, HdrEncodeErrors};

mod decoder;
mod encoder;
mod errors;
