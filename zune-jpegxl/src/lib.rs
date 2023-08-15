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
//! compared png for non-photo content and much better for other situations
//!
//!  # Features
//!  
//!  - `std`: Enables linking against the  standard library
//!  - `threads`: Enables using the standard library threading capabilities, this feature requires
//!     the `std` feature to work (threading doesn't exist in no-std), if the above feature isn't enabled
//!     this is a no-op
//!
//! Both features are enabled by default.
//!
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
