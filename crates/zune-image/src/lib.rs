/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! A fast and simple image processing library
//!
//! This ties up most of the independent crates and provides functionality
//! between each one
//!
//!
//! ## Features
//! - The library crates include features for various formats anf filters
//! - Decoders and encoders can be included or excluded at will
//!
//! ### Image decoders and encoders
//! By default, a feature includes both format decoder and encoder if present.
//!
//!
//!| Feature      | Decoder       | Encoder        |
//!|--------------|---------------|----------------|
//!| bmp          | zune-bmp      |     -          |
//!| jpeg         | zune-jpeg     | [jpeg-encoder] |
//!| png          | zune-png      | zune-png       |
//!| ppm          | zune-ppm      | zune-ppm       |
//!| qoi          | zune-qoi      | zune-qoi       |
//!| farbfeld     | zune-farbfeld | zune-farbfeld  |
//!| psd          | zune-psd      | -              |
//!| jpeg-xl      | [jxl-oxide]   | zune-jpegxl    |
//!| hdr          | zune-hdr      | zune-hdr       |
//!
//!
//! ### Image filters
//!
//! Image filters are divided into two types,
//!  - core filters: Things needed to enable conversions from one format to another.
//!                 This may include color conversion and depth conversion routines.
//!                 These are in `zune-image` crate
//!  - extra filters: This include algorithms that do more complex pixel manipulations,
//!     including contrast adjustment, resizing, blurring etc, the algorithms are usually
//!     implemented in `zune-imageprocs` by the processes implementing
//!     [OperationsTrait](crate::traits::OperationsTrait)
//!
//!  # High level api
//! Load images using image `open`
//!
//!```no_run
//! use zune_image::errors::ImageErrors;
//! use zune_image::image::Image;
//!        
//! let image = Image::open("file.png")?;
//!
//!# Ok::<(),ImageErrors>(())
//! ```
//!  Or if the image is in memory load it via [`Image.read`](crate::image::Image::read)
//!
//!```no_run
//! use zune_core::bytestream::ZCursor;
//! use zune_core::options::DecoderOptions;
//! use zune_image::image::Image;
//! use zune_image::errors::ImageErrors;
//! let mem_src = [0;100];
//! let image = Image::read(ZCursor::new(&mem_src),DecoderOptions::default())?;
//! # Ok::<(),ImageErrors>(())
//!
//! ```
//! You can save files via [`Image.save`](crate::image::Image::save)
//! which takes a file name and uses the extension to determine the file type, or
//! [`Image.save_to`](crate::image::Image::save_to) which takes an additional format field
//! or [`Image.write_to_vec`](crate::image::Image::write_to_vec) which writes image contents to memory
//! locations
//!
//!
//!  ### Image and frames
//! An image may consist of one or more frames, an image with more than one frame is considered
//! animated, each frame of an animated image should have the same color channels and length.
//!
//! You can iterate the frames via the `frames_` method ([`frames_ref`](image::Image::frames_ref)
//! and [`frames_mut`](image::Image::frames_mut)
//!
//! ### Image and channels
//!
//! The channels api ([`channels_ref`](image::Image::channels_ref) and [`channels_mut`](image::Image::channels_mut) provide
//! convenient methods to access image channels. This returns all image channels,traversing frames and concatenating it together
//!
//!
//![image]:https://crates.io/crates/image
//! [jpeg-encoder]: https://crates.io/crates/jpeg-encoder
//! [jxl-oxide]: https://crates.io/crates/jxl-oxide
#![allow(
    clippy::redundant_field_names,
    clippy::uninlined_format_args,
    rustdoc::redundant_explicit_links
)]
extern crate core;

pub mod channel;
pub mod codecs;
pub mod core_filters;
mod deinterleave;
pub mod errors;
pub mod frame;
pub mod image;
pub mod metadata;
mod ops;
pub mod pipelines;
mod serde;
mod tests;
pub mod traits;
pub mod utils;
