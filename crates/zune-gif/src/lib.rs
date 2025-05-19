#![no_std]

#[macro_use]
extern crate alloc;
extern crate core;

mod decoder;
mod enums;
mod errors;

pub use decoder::GifDecoder;
pub use errors::GifDecoderErrors;
