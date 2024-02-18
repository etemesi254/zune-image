/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Traits for reading and writing images in zune
//!
//!
//! This exposes the traits and implementations for readers
//! and writers in the zune family of decoders and encoders.

use crate::bytestream::reader::{ZByteIoError, ZSeekFrom};

pub trait ZByteIoTrait {
    fn read_byte_no_error(&mut self) -> u8;
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError>;
    fn read_const_bytes<const N: usize>(&mut self, buf: &mut [u8; N]) -> Result<(), ZByteIoError>;
    fn read_const_bytes_no_error<const N: usize>(&mut self, buf: &mut [u8; N]);
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError>;
    /// Reads data into provided buffer but does not advance read position.
    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError>;
    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError>;
    /// Reads single byte from the stream.
    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError>;
    /// Tells whether this is end of stream.
    fn is_eof(&mut self) -> Result<bool, ZByteIoError>;
    /// Returns stream size or -1 if it is not known.
    fn z_size(&mut self) -> Result<i64, ZByteIoError>;
    /// The name of the impl
    fn name(&self) -> &'static str;
    fn z_position(&mut self) -> Result<u64, ZByteIoError>;
    fn read_remaining(&mut self, sink: &mut alloc::vec::Vec<u8>) -> Result<usize, ZByteIoError>;
}
