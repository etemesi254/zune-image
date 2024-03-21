/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

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
//! compared png for non-photo content and much better for other situations.
//!
//! The library is also fully safe
//!
//!  # Features
//!  
//!  - `std`: Enables linking against the  standard library
//!  - `threads`: Enables using the standard library threading capabilities, this feature requires
//!     the `std` feature to work (threading doesn't exist in no-std), if the above feature isn't enabled
//!     this is a no-op
//!  - `log`:  Enables use of `log` to report on encoding configs and status
//!
//!
//! Both features are enabled by default.
//!
//!  # 16 bit data
//! - 16 bit data should be reinterpreted as 2 u8's in `native endian`,
//!
//!
//!
//!  
//! # Example
//! - Encode a 2x2 8 bit image
//! ```
//! use zune_core::bit_depth::BitDepth;
//! use zune_core::colorspace::ColorSpace;
//! use zune_core::options::EncoderOptions;
//! use zune_jpegxl::JxlSimpleEncoder;
//! use zune_jpegxl::JxlEncodeErrors;
//! // encode a 2x2 image
//!
//! fn main()->Result<(),JxlEncodeErrors>{
//!     let mut encoder = JxlSimpleEncoder::new(&[0,0,0,0],EncoderOptions::new(2,2,ColorSpace::Luma,BitDepth::Eight));
//! let mut write_to = vec![];
//!     encoder.encode(&mut write_to)?;
//!     Ok(())
//! }
//! ```
//! - Encode a 2x2 16 bit image
//! ```
//! use zune_core::bit_depth::BitDepth;
//! use zune_core::colorspace::ColorSpace;
//! use zune_core::options::EncoderOptions;
//! use zune_jpegxl::JxlSimpleEncoder;
//! use zune_jpegxl::JxlEncodeErrors;
//! // encode a 2x2 image
//!
//! fn main()->Result<(),JxlEncodeErrors>{
//!     // convert a 16 bit input to 8 bit native endian output, each two bytes represent one sample
//!     let sixteen_bit = [0,u16::MAX,0,u16::MAX].iter().flat_map(|x| x.to_ne_bytes()).collect::<Vec<u8>>();
//!     let mut encoder = JxlSimpleEncoder::new(&sixteen_bit,EncoderOptions::new(2,2,ColorSpace::Luma,BitDepth::Sixteen));
//!     let mut write_to = vec![];
//!     encoder.encode(&mut write_to)?;
//!     Ok(())
//! }
//! ```
//!
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]
#![macro_use]
extern crate alloc;
extern crate core;

pub use encoder::JxlSimpleEncoder;
pub use errors::JxlEncodeErrors;

mod bit_depth;
mod bit_writer;
mod color_convert;
mod encoder;
mod errors;
