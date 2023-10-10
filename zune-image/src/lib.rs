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
//!| jpeg-xl      | -             | zune-jpegxl    |
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
//!     implemented in `zune-imageprocs` and the glue between algorithms and image is done in image
//!     [filters](crate::filters)
//!
//! # Things the library is not good at.
//!
//! - Per pixel access methods, while there are methods provided for
//! such things such as [from_fn](crate::image::Image::from_fn) and
//! [modify_pixels_mut](crate::image::Image::modify_pixels_mut) the images
//! are represented as planar hence there will be a lot of cache misses as opposed
//! to others that represents pixels as one single continuous buffer.
//!
//! If you plan on doing multiple per pixel manipulations, [image] crate may be a better
//! fit for your needs
//!
//![image]:https://crates.io/crates/image
//! [jpeg-encoder]: https://crates.io/crates/jpeg-encoder
#![allow(clippy::redundant_field_names, clippy::uninlined_format_args)]
extern crate core;

pub mod channel;
pub mod codecs;
pub mod core_filters;
pub mod deinterleave;
pub mod errors;
pub mod filters;
pub mod frame;
pub mod image;
pub mod metadata;
mod ops;
mod serde;
mod tests;
pub mod traits;
pub mod workflow;
