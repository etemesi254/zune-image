//! An incredibly spiffy deflate decoder
pub use crate::decoder::ZlibDecoder;

mod bitstream;
mod constants;
mod decoder;
mod enums;
pub mod errors;
mod utils;
