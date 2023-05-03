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
pub use encoder::HdrEncoder;
pub use errors::{HdrDecodeErrors, HdrEncodeErrors};
pub use zune_core;

mod decoder;
mod encoder;
mod errors;
