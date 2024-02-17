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

use alloc::vec::Vec;
use core::ops::Range;

use crate::bytestream::{ZByteIoError, ZSeekFrom};

/// The underlying reader trait
///
/// # Considerations
///
///- When implementing this for a type, it is recommended to implement methods with
/// `#inline[(always)]` directive to allow the functions to get inlined in call sites,
/// this may make it faster on some situations since the call sites may be in hot loop.
///
/// - If you are reading from a file and it's small , it is preferable to read it into memory
/// instead of using a file reader.
pub trait ZReaderTrait {
    /// Get a single byte which is at position `index`
    ///
    /// # Arguments
    /// - `index`: The position of the bytes
    fn get_byte(&self, index: usize) -> Option<&u8>;

    /// Get a slice of bytes from a range of start..end
    ///
    /// # Arguments
    ///
    /// * `index`:  The range of the bytes to read
    ///
    /// returns: `Option<&[u8]>`
    ///
    /// # Examples
    ///
    /// - Read 10 bytes from
    /// ```
    /// extern crate alloc;
    /// use alloc::vec::Vec;
    /// use zune_core::bytestream::ZReaderTrait;
    ///
    /// let bytes = vec![0_u8;100];
    ///
    /// // get ten bytes from 0..10
    /// let re = bytes.get_slice(0..10).unwrap();
    /// assert_eq!(10,re.len())
    ///
    /// ```
    fn get_slice(&self, index: Range<usize>) -> Option<&[u8]>;

    /// Get total length of the underlying buffer.
    ///
    /// This should be the total bytes that are present in
    /// the buffer.
    ///
    /// For files, this includes the file  length.
    /// For buffers this includes the internal buffer length
    fn get_len(&self) -> usize;
}

impl ZReaderTrait for &[u8] {
    #[inline(always)]
    fn get_byte(&self, index: usize) -> Option<&u8> {
        self.get(index)
    }

    #[inline(always)]
    fn get_slice(&self, index: Range<usize>) -> Option<&[u8]> {
        self.get(index)
    }

    #[inline(always)]
    fn get_len(&self) -> usize {
        self.len()
    }
}

impl ZReaderTrait for Vec<u8> {
    #[inline(always)]
    fn get_byte(&self, index: usize) -> Option<&u8> {
        self.get(index)
    }

    #[inline(always)]
    fn get_slice(&self, index: Range<usize>) -> Option<&[u8]> {
        self.get(index)
    }

    #[inline(always)]
    fn get_len(&self) -> usize {
        self.len()
    }
}

impl ZReaderTrait for &Vec<u8> {
    #[inline(always)]
    fn get_byte(&self, index: usize) -> Option<&u8> {
        self.get(index)
    }

    #[inline(always)]
    fn get_slice(&self, index: Range<usize>) -> Option<&[u8]> {
        self.get(index)
    }

    #[inline(always)]
    fn get_len(&self) -> usize {
        self.len()
    }
}

impl<const N: usize> ZReaderTrait for &[u8; N] {
    fn get_byte(&self, index: usize) -> Option<&u8> {
        self.get(index)
    }

    fn get_slice(&self, index: Range<usize>) -> Option<&[u8]> {
        self.get(index)
    }

    fn get_len(&self) -> usize {
        N
    }
}

impl<const N: usize> ZReaderTrait for [u8; N] {
    fn get_byte(&self, index: usize) -> Option<&u8> {
        self.get(index)
    }

    fn get_slice(&self, index: Range<usize>) -> Option<&[u8]> {
        self.get(index)
    }

    fn get_len(&self) -> usize {
        N
    }
}

impl ZReaderTrait for dyn AsRef<&[u8]> {
    fn get_byte(&self, index: usize) -> Option<&u8> {
        self.as_ref().get(index)
    }

    fn get_slice(&self, index: Range<usize>) -> Option<&[u8]> {
        self.as_ref().get(index)
    }

    fn get_len(&self) -> usize {
        self.as_ref().len()
    }
}

pub trait ZByteIoTrait {
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError>;
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
