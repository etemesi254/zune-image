#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unexpected_cfgs)]
// The decoder itself is safe Rust; `unsafe` is only allowed in the opt-in
// platform modules below (SIMD transforms, Apple VideoToolbox FFI).
#![deny(unsafe_code)]
#![cfg_attr(feature = "simd", feature(portable_simd))]
#![warn(
    clippy::correctness,
    clippy::perf,
    clippy::pedantic,
    clippy::inline_always,
    clippy::missing_errors_doc,
    clippy::panic
)]
#![allow(
    clippy::needless_return,
    clippy::similar_names,
    clippy::inline_always,
    clippy::needless_range_loop,
    clippy::doc_markdown,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::struct_excessive_bools
)]
#[macro_use]
extern crate alloc;
extern crate core;
#[cfg(feature = "std")]
#[allow(unsafe_code)] // FFI to Apple VideoToolbox
mod apple_videotoolbox;
mod decoder;
mod errors;
mod header_structs;
mod headers;
pub mod hevc_decoder;
mod processor;
mod utils;

pub extern crate zune_core;

pub use decoder::HeifDecoder;
pub use errors::HeicErrors;
