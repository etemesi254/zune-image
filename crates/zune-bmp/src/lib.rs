/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! A versatile BMP decoder
//!
//! This crate features a BMP decoder capable of decoding
//! multiple BMP images fast
//!
//! # Features
//! - `no_std` by default with `alloc` feature
//! - Fast
//! - Minimal dependencies
//! - Very minimal internal allocation. (most paths do not allocate any more than output buffer)
//! # Supported formats
//! - RLE (4 bit and 8 bit)
//! - Paletted images(1 bit, 2 bits, 4  bits and 8 bits)
//! - Masked images (16 bit and 32 bit formats)
//!
//! # Unsupported formats
//! - Embedded PNG and JPEGs
//!
//!
//! # Security
//!
//! The decoder is continuously fuzz tested in CI to ensure it does not crash on malicious input
//! in case a sample causes it to crash, an issue would be welcome.

// failing tests
// pal4rlecut.bmp
// pal8rlecut.bmp
// q/rgb24rle24.bmp
//

#![no_std]
#![macro_use]
extern crate alloc;

pub use crate::decoder::{probe_bmp, BmpDecoder};
pub use crate::errors::BmpDecoderErrors;

mod common;
mod decoder;
mod errors;
mod utils;
