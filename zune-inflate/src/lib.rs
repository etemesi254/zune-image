//! An incredibly spiffy deflate decoder
pub use crate::decoder::DeflateDecoder;

mod bitstream;
mod constants;
mod decoder;
pub mod errors;
mod utils;
