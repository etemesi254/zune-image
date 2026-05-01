#![allow(unexpected_cfgs)]
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
extern crate alloc;
extern crate core;

mod apple_videotoolbox;
mod bmf_reader;
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
