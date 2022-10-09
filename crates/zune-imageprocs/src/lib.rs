// Benchmark support needs sse
#![cfg_attr(feature = "benchmarks", feature(test))]

pub mod deinterleave;
pub mod grayscale;
pub mod transpose;
