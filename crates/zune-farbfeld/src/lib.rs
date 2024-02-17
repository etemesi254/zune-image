/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! farbfeld is a lossless image format which is easy to parse, pipe and compress. It has the following format:
//! ```text
//! ╔════════╤═════════════════════════════════════════════════════════╗
//! ║ Bytes  │ Description                                             ║
//! ╠════════╪═════════════════════════════════════════════════════════╣
//! ║ 8      │ "farbfeld" magic value                                  ║
//! ╟────────┼─────────────────────────────────────────────────────────╢
//! ║ 4      │ 32-Bit BE unsigned integer (width)                      ║
//! ╟────────┼─────────────────────────────────────────────────────────╢
//! ║ 4      │ 32-Bit BE unsigned integer (height)                     ║
//! ╟────────┼─────────────────────────────────────────────────────────╢
//! ║ [2222] │ 4x16-Bit BE unsigned integers [RGBA] / pixel, row-major ║
//! ╚════════╧═════════════════════════════════════════════════════════╝
//! The RGB-data should be sRGB for best interoperability and not alpha-premultiplied.
//!```
//!
//!
#![no_std]
#![macro_use]
extern crate alloc;

pub use decoder::*;
pub use encoder::*;

mod decoder;
mod encoder;
mod errors;
