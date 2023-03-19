//! Zune-jpegxl
//!
//! Support for encoding jpeg xl images in pure rust
//!
//! This currently features a small POC encoder for JPEG-XL
//! format based on the POC over at the libjxl crate
//!
//! It supports the following features
//!
//! - lossless compression
//! - up to 16 bits of depth
//! - Up to 4 channels for images
//!
//! - Non supported features
//!  -> Palette support
//!
//! Currently, it's fast with slightly worse compression when
//! compared png for non-photo content and much better for other situations
//!
//!
//! # Example
//! ```no_run
//! use zune_core::bit_depth::BitDepth;
//! use zune_core::options::EncoderOptions;
//! use zune_jpegxl::JxlSimpleEncoder;
//! let mut encoder = JxlSimpleEncoder::new(b"Hello world",EncoderOptions::default());
//! encoder.encode().unwrap();
//! ```
//!
pub use encoder::JxlSimpleEncoder;
pub use errors::JxlEncodeErrors;

mod bit_depth;
mod bit_writer;
mod color_convert;
mod encoder;
mod errors;
