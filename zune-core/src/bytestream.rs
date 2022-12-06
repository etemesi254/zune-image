//! A simple implementation of a bytestream reader
//! and writer.
//!
//! This module contains two main structs that help in
//! byte reading and byte writing
//!
//! Useful for a lot of image readers and writers, it's put
//! here to minimize code reuse
pub use reader::ZByteReader;

mod reader;
mod writer;
