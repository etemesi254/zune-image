/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Bit I/O functionalities

use alloc::vec;
use alloc::vec::Vec;

use zune_core::bytestream::{ZByteIoError, ZWriter, ZByteWriterTrait};

/// Construct a new bit writer
/// This bit writer owns it's output and you need to call
/// `allocate` before using it
#[derive(Clone, Debug)]
pub struct BitWriter {
    pub bits_in_buffer: u8,
    pub buffer:         u64,
    pub position:       usize,
    pub dest:           Vec<u8>
}

impl BitWriter {
    /// Write pending bits into the output buffer
    ///
    /// This may leave between 0-7 bits remaining in the bit buffer
    pub(crate) fn flush(&mut self) {
        let buf = self.buffer.to_le_bytes();
        // write 8 bytes
        self.dest[self.position..self.position + 8].copy_from_slice(&buf);
        // but update position to point to the full number of symbols we read
        let bytes_written = self.bits_in_buffer & 56;

        // remove those bits we wrote.
        self.buffer >>= bytes_written;
        // increment position
        self.position += (bytes_written >> 3) as usize;

        self.bits_in_buffer &= 7;
    }

    /// Construct a new bit-writer
    pub fn new() -> BitWriter {
        BitWriter {
            bits_in_buffer: 0,
            buffer:         0,
            position:       0,
            dest:           vec![]
        }
    }

    /// Create space for the bits which will be stored
    ///
    /// This adds padding bytes, so do not treat it as a full
    /// stream
    ///
    /// # Arguments
    /// - maximum_bit_size: Maximum expected bits which will
    /// be stored in this decoder
    pub fn allocate(&mut self, maximum_bit_size: usize) {
        assert!(self.dest.is_empty());
        self.dest.resize(maximum_bit_size / 8 + 64, 0);
    }
    /// Put some bits to the buffer
    /// And periodically flush to output when necessary
    ///
    /// # Arguments
    /// - nbits: Number of bits to store in the buffer
    /// - bit: The bits in the buffer
    pub fn put_bits(&mut self, nbits: u8, bit: u64) {
        debug_assert!(nbits < 56);

        if self.bits_in_buffer + nbits > 56 {
            self.flush();
        }
        // still check, because I don't trust myself
        debug_assert!(nbits + self.bits_in_buffer < 64);

        let mask = (1 << nbits) - 1;

        // add to the top of the bit buffer
        self.buffer |= (mask & bit) << self.bits_in_buffer;
        self.bits_in_buffer += nbits;
    }

    /// Pad bytes to  be zero aligned
    pub fn zero_pad(&mut self) {
        // flush output first
        self.flush();

        if self.bits_in_buffer != 0 {
            self.put_bits(8 - self.bits_in_buffer, 0);
        }
    }
}

/// A bit writer that uses an already given output
/// array to write bits into
pub struct BorrowingBitWriter<'a, T: ZByteWriterTrait> {
    pub bits_in_buffer: u8,
    pub buffer:         u64,
    pub position:       usize,
    pub dest:           &'a mut ZWriter<T>
}

impl<'a, T: ZByteWriterTrait> BorrowingBitWriter<'a, T> {
    /// Write pending bits to the buffer
    pub(crate) fn flush(&mut self) -> Result<(), ZByteIoError> {
        let buf = self.buffer.to_le_bytes();
        // write 8 bytes
        //self.dest[self.position..self.position + 8].copy_from_slice(&buf);
        // but update position to point to the full number of symbols we read
        let bits_written = self.bits_in_buffer & 56;

        let bytes_written = usize::from(bits_written >> 3);
        // remove those bits we wrote.
        self.buffer >>= bits_written;
        // increment position
        self.position += bytes_written;
        // potentially expensive call
        self.dest.write_all(&buf[..bytes_written])?;

        self.bits_in_buffer &= 7;
        Ok(())
    }

    /// Construct a new bit-writer
    pub fn new(data: &'a mut ZWriter<T>) -> BorrowingBitWriter<'a, T> {
        BorrowingBitWriter {
            bits_in_buffer: 0,
            buffer:         0,
            position:       0,
            dest:           data
        }
    }

    /// Put some bits to the buffer
    /// And periodically flush to output when necessary
    pub fn put_bits(&mut self, nbits: u8, bit: u64) -> Result<(), ZByteIoError> {
        debug_assert!(nbits <= 56);

        if self.bits_in_buffer + nbits > 56 {
            self.flush()?;
        }
        // still check, because I don't trust myself
        debug_assert!(nbits + self.bits_in_buffer < 64);

        let mask = (1 << nbits) - 1;

        // add to the top of the bit buffer
        self.buffer |= (mask & bit) << self.bits_in_buffer;
        self.bits_in_buffer += nbits;
        Ok(())
    }
    /// Put some bits to the buffer
    /// Without flushing
    pub fn put_bits_no_flush(&mut self, nbits: u8, bit: u64) {
        debug_assert!(nbits <= 56);

        // still check, because I don't trust myself
        debug_assert!(nbits + self.bits_in_buffer < 64);

        let mask = (1 << nbits) - 1;

        // add to the top of the bit buffer
        self.buffer |= (mask & bit) << self.bits_in_buffer;
        self.bits_in_buffer += nbits;
    }

    pub fn put_bytes(&mut self, bytes: &[u8]) -> Result<(), ZByteIoError> {
        //check if we can simply copy from input to output
        // when we have aligned bits, that becomes possible
        if (self.bits_in_buffer & 7) == 0 {
            // flush any pending bits
            self.flush()?;
            // ensure no bits are present
            assert_eq!(self.bits_in_buffer, 0);
            // copy
            self.dest.write_all(bytes)?;
            // update position
            self.position += bytes.len();
        } else {
            // unaligned bytes
            // we want to write as many bytes as possible
            // so we chunk input into 48 bits/6 bytes
            // copy them per invocation

            // To create space, flush bits first
            self.flush()?;

            let mut buffer = [0; 8];

            const SINGLE_CHUNK: usize = 7;
            // now chunk
            // fast loop that handles exact byte chunks
            for chunks in bytes.chunks_exact(SINGLE_CHUNK) {
                // we need them in LE format
                // msb at the bottom
                buffer[..SINGLE_CHUNK].copy_from_slice(chunks);
                // now add bits
                self.put_bits_no_flush(SINGLE_CHUNK as u8 * 8, u64::from_le_bytes(buffer));
                self.flush()?;
            }

            // in case of a remainder, means the byte length
            // wasn't divisible evenly by 6.
            // so we handle the remainder the same way.
            // the only reason why this is separate from above
            // is that it would have required
            // more book-keeping above
            let remainder = bytes.chunks_exact(SINGLE_CHUNK).remainder();

            if !remainder.is_empty() {
                // handle remainder bytes
                buffer.fill(0);

                buffer[..remainder.len()].copy_from_slice(remainder);
                self.put_bits((remainder.len() * 8) as u8, u64::from_le_bytes(buffer))?;
            }
            self.flush()?;
        }
        Ok(())
    }
}

#[rustfmt::skip]
#[inline]
pub fn encode_hybrid_uint_lz77(value: u32, token: &mut u32, nbits: &mut u32, bits: &mut u32)
{
    let n = 31 - ((value | 1).leading_zeros());

    *token = if value < 16 { value } else { 16 + n - 4 };
    *nbits = if value < 16 { 0 } else { n };
    *bits = if value < 16 { 0 } else { value - (1 << *nbits) };
}

#[rustfmt::skip]
#[inline]
pub fn encode_hybrid_unit_000(value: u32, token: &mut u32, nbits: &mut u32, bits: &mut u32)
{
    let n = 31 - ((value | 1).leading_zeros());

    *token = if value != 0 { n + 1 } else { 0 };
    *nbits = if value != 0 { n } else { 0 };
    *bits = if value != 0 { value - (1 << n) } else { 0 };
}
