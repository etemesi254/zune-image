//!A png decoder
//!
//! # Using
//!
//! Below is an example of how to decode images
//! ```no_run
//! use zune_png::PngDecoder;
//! use zune_core::DecodingResult;
//! let mut decoder = PngDecoder::new(&[]);
//!
//! let pixels = decoder.decode().unwrap();
//! let value=match pixels {
//!
//!     DecodingResult::U8(value)=>{
//!         // you got  a u8
//!     }
//!     DecodingResult::U16(value)=>{
//!         // you got a u16, squeeze it to u8?
//!     }
//! };
//! ```
//!
pub use decoder::PngDecoder;

mod constants;
mod crc;
mod decoder;
mod enums;
pub mod error;
mod filters;
mod gamma_correct;
mod headers;
mod options;
