#![feature(portable_simd)]
#![allow(unexpected_cfgs)]
#![cfg_attr(feature = "portable-simd", feature(portable_simd))]
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
    clippy::similar_names,
    clippy::doc_markdown,
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
