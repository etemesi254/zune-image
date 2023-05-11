/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

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

pub mod box_blur;
pub mod brighten;
pub mod contrast;
pub mod convolve;
pub mod crop;
pub mod flip;
pub mod flop;
pub mod gamma;
pub mod gaussian_blur;
pub mod invert;
pub mod mathops;
pub mod median;
pub mod mirror;
pub mod pad;
pub mod premul_alpha;
pub mod prewitt;
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
