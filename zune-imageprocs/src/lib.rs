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
pub mod convolve;
pub mod crop;
pub mod deinterleave;
pub mod flip;
pub mod flop;
pub mod gamma;
pub mod gaussian_blur;
pub mod grayscale;
pub mod invert;
pub mod mathops;
pub mod median;
pub mod mirror;
pub mod pad;
pub mod rotate;
pub mod spatial;
pub mod spatial_ops;
pub mod stretch_contrast;
pub mod threshold;
pub mod traits;
pub mod transpose;
pub mod unsharpen;
mod utils;
