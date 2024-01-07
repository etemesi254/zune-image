#![cfg_attr(feature = "portable-simd", feature(portable_simd))]
/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Image processing routines for `zune-image`
//!
//! This implements some common image processing routines to be used with `zune-image`
//!
//! It implements the `OperationsTrait` defined by zune-image.
//!
//! # Example
//! - Increase exposure of image by 2.0
//! ```
//! use zune_core::colorspace::ColorSpace;
//! use zune_image::image::Image;
//! use zune_image::traits::OperationsTrait;
//! use zune_imageprocs::exposure::Exposure;
//! let mut image = Image::fill::<u8>(233,ColorSpace::RGB,100,100);
//! let exposure = Exposure::new(2.0,0.0);
//! // execute the filter
//! exposure.execute(&mut image).unwrap();
//! ```

// Benchmark support needs sse
#![cfg_attr(feature = "benchmarks", feature(test))]
#![warn(
    clippy::correctness,
    clippy::perf,
    clippy::pedantic,
    clippy::inline_always,
    clippy::missing_errors_doc,
    clippy::panic
)]
#![allow(
    clippy::needless_return,
    clippy::similar_names,
    clippy::inline_always,
    clippy::similar_names,
    clippy::doc_markdown,
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::wildcard_imports
)]

pub mod auto_orient;
pub mod bilateral_filter;
pub mod blend;
pub mod box_blur;
pub mod brighten;
pub mod color_matrix;
pub mod composite;
pub mod contrast;
pub mod convolve;
pub mod crop;
pub mod exposure;
pub mod flip;
pub mod flop;
pub mod gamma;
pub mod gaussian_blur;
pub mod histogram;
pub mod hsv_adjust;
pub mod invert;
pub mod mathops;
pub mod median;
pub mod mirror;
pub mod pad;
pub mod premul_alpha;
mod prewitt;
pub mod resize;
pub mod rotate;
pub mod scharr;
pub mod sobel;
pub mod spatial;
pub mod spatial_ops;
pub mod stretch_contrast;
pub mod threshold;
pub mod traits;
pub mod transpose;
pub mod unsharpen;
mod utils;
