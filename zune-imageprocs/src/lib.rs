// Benchmark support needs sse
#![cfg(feature = "benchmarks")]
#![feature(test)]

pub mod deinterleave;
pub mod grayscale;
pub mod transpose;
