#![allow(unexpected_cfgs)]
extern crate core;
extern crate alloc;

mod apple_videotoolbox;
mod bmf_reader;
mod decoder;
mod errors;
mod header_structs;
mod headers;
mod processor;
mod utils;
pub mod hevc_decoder;

pub use decoder::HeifDecoder;
pub use errors::HeicErrors;
