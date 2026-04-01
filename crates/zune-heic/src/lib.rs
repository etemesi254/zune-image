#![feature(portable_simd)]
#![allow(unexpected_cfgs)]
#![cfg_attr(feature = "portable-simd", feature(portable_simd))]

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

pub use decoder::HeifDecoder;
pub use errors::HeicErrors;
