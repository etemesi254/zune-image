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

/// The de-facto Input trait implemented for readers.
///
/// This provides the basic functions needed to quick and sometimes
/// heap free I/O for the zune image decoders with easy support for extending it
/// to multiple implementations.
pub trait ZByteReaderTrait {
    /// Read a single byte from the decoder and return
    /// `0` if we can't read the byte, e.g because of EOF
    ///
    /// The implementation should try to be as fast as possible as this is called
    /// from some hot loops where it may become the bottleneck
    fn read_byte_no_error(&mut self) -> u8;
    /// Read exact bytes required to fill `buf` or return an error if that isn't possible
    ///
    /// ## Arguments
    ///  - `buf`: Buffer to fill with bytes from the underlying reader
    ///  ## Errors
    /// In case of an error, the implementation should not increment the internal position
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError>;

    /// Read exact bytes required to fill `buf` or return an error if that isn't possible
    ///
    /// This is the same as [`read_exact_bytes`](Self::read_exact_bytes) but implemented as a separate
    /// method to allow some implementations to optimize it to cost fewer instructions
    ///
    /// ## Arguments
    ///  - `buf`: Buffer to fill with bytes from the underlying reader
    ///  ## Errors
    /// In case of an error, the implementation should not increment the internal position
    fn read_const_bytes<const N: usize>(&mut self, buf: &mut [u8; N]) -> Result<(), ZByteIoError>;

    /// Read exact bytes required to fill `buf` or ignore buf entirely if you can't fill it
    /// due to an error like the inability to fill the buffer completely
    /// ## Arguments
    ///  - `buf`: Buffer to fill with bytes from the underlying reader
    /// ## Errors
    /// In case of an error, the implementation should not increment the internal position
    fn read_const_bytes_no_error<const N: usize>(&mut self, buf: &mut [u8; N]);

    /// Read bytes into `buf` returning how many bytes you have read or an error if one occurred
    ///
    /// This doesn't guarantee that buf will be filled with bytes for such a guarantee see
    /// [`read_exact_bytes`](Self::read_exact_bytes)
    ///
    /// ## Arguments
    /// - `buf`: The buffer to fill with bytes
    ///
    /// ## Returns
    ///  - `Ok(usize)` - Actual bytes read into the buffer
    ///  - `Err()` - The error encountered when reading bytes for which we couldn't recover
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError>;
    /// Reads data into provided buffer but does not advance read position.
    ///
    ///
    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError>;
    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError>;
    /// Seek into a new position from the buffer
    ///
    /// This is similar to the [seek](std::io::Seek::seek) function in the [Seek](std::io::Seek) trait
    /// but implemented to work for no-std environments
    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError>;
    /// Report whether we are at the end of a stream.
    ///
    /// ## Warning
    /// This may cause an additional syscall e.g when we are reading from a file, we must query the file
    /// multiple times to check if we really are at the end of the file and the user didn't sneakily
    /// add more contents to it hence use it with care
    ///
    /// ## Returns
    /// - `Ok(bool)` - The answer to whether or not we are at end of file
    /// - `Err()` - The error that occurred when we queried the underlying reader if we were at EOF
    fn is_eof(&mut self) -> Result<bool, ZByteIoError>;

    /// Return the current position of the inner cursor.
    ///
    /// This can be used to check the advancement of the cursor
    fn z_position(&mut self) -> Result<u64, ZByteIoError>;
    /// Read all bytes remaining in this input to `sink` until we hit eof
    ///
    /// # Returns
    /// - `Ok(usize)` The actual number of bytes added to the sink
    /// - `Err()` An error that occurred when reading bytes
    fn read_remaining(&mut self, sink: &mut alloc::vec::Vec<u8>) -> Result<usize, ZByteIoError>;
}

pub trait ZByteWriterTrait {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<usize, ZByteIoError>;
    fn write_all_bytes(&mut self, buf: &[u8]) -> Result<(), ZByteIoError>;
    fn write_const_bytes<const N: usize>(&mut self, buf: &[u8; N]) -> Result<(), ZByteIoError>;
    fn flush_bytes(&mut self) -> Result<(), ZByteIoError>;

    fn pre_allocate_hint(&mut self, size: usize) -> Result<(), ZByteIoError>;
}
