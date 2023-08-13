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
//!
#![no_std]
#![macro_use]
extern crate alloc;

pub use crate::decoder::{probe_bmp, BmpDecoder};
pub use crate::errors::BmpDecoderErrors;

mod common;
mod decoder;
mod errors;
mod utils;
