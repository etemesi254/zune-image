//! farbfeld is a lossless image format which is easy to parse, pipe and compress. It has the following format:
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
//!

pub use decoder::*;

mod decoder;
