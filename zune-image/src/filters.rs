/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Contains image manipulation algorithms
//!
//! This contains structs that implement `OperationsTrait`
//! meaning they can manipulate images
#![cfg(feature = "filters")]
pub mod box_blur;
pub mod brighten;
pub mod contrast;
pub mod convolve;
pub mod crop;
pub mod exposure;
pub mod flip;
pub mod flop;
pub mod gamma;
pub mod gaussian_blur;
pub mod invert;
pub mod median;
pub mod mirror;
pub mod orientation;
pub mod premul_alpha;
pub mod resize;
pub mod rotate;
pub mod scharr;
pub mod sobel;
pub mod statistics;
pub mod stretch_contrast;
pub mod threshold;
pub mod transpose;
pub mod unsharpen;
