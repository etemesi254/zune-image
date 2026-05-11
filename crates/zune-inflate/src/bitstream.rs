/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! `BitStreamReader` API
//!
//! This module provides an interface to read and write bits (and bytes) for
//! huffman

pub struct BitStreamReader<'src> {
    // buffer from which we are pulling in bits from
    // used in decompression.
    pub src: &'src [u8],
    // position in our buffer,
    pub position: usize,
    pub bits_left: u8,
    pub buffer: u64,
    pub over_read: usize,
    // Indicates if this is the last chunk of the stream.
    pub is_final: bool,
}

impl<'src> BitStreamReader<'src> {
    /// Create a new `BitStreamReader` instance
    ///
    pub fn new(in_buffer: &'src [u8], is_final: bool) -> BitStreamReader<'src> {
        BitStreamReader {
            bits_left: 0,
            buffer: 0,
            src: in_buffer,
            position: 0,
            over_read: 0,
            is_final,
        }
    }
    /// Update the reader with a new chunk of data.
    /// This preserves `bits_left` and `buffer` so we can resume
    /// exactly where a code crossed the chunk boundary.
    pub fn update_chunk(&mut self, new_chunk: &'src [u8], is_final: bool) {
        if self.src.as_ptr() != new_chunk.as_ptr()  {
            self.src = new_chunk;
            self.position = 0;
        }
        self.is_final = is_final;

        // Wipe out any "Ghost Bytes" trapped in the buffer
        // from the previous chunk before they corrupt the new chunk!
        self.buffer &= (1_u64 << self.bits_left) - 1;
    }
    /// Refill the bitstream ensuring the buffer has bits between
    /// 56 and 63.
    ///
    #[inline(always)]
    pub fn refill(&mut self) {
        /*
         * The refill always guarantees refills between 56-63
         *
         * Bits stored will never go above 63 and if bits are in the range 56-63 no refills occur.
         */
        let mut buf = [0; 8];

        match self.src.get(self.position..self.position + 8) {
            Some(bytes) => {
                buf.copy_from_slice(bytes);
                // create a u64 from an array of u8's
                let new_buffer = u64::from_le_bytes(buf);
                // num indicates how many bytes we actually consumed.
                let num = 63 ^ self.bits_left;
                // offset position
                self.position += (num >> 3) as usize;
                // shift number of bits
                self.buffer |= new_buffer << self.bits_left;
                // update bits left
                // bits left are now between 56-63
                self.bits_left |= 56;
            }
            None => self.refill_slow(),
        }
    }
    #[inline(never)]
    fn refill_slow(&mut self) {
        let bytes = &self.src[self.position..];

        for byte in bytes {
            if self.bits_left >= 56 {
                break;
            }

            self.buffer |= u64::from(*byte) << self.bits_left;
            self.bits_left += 8;
            self.position += 1;
        }

        // ONLY pad with dummy zeros if we are absolutely sure
        // there is no more data coming.
        if self.is_final {
            while self.bits_left < 56 {
                self.bits_left += 8;
                self.over_read += 1;
            }
        }
    }

    #[inline(always)]
    pub fn peek_bits<const LOOKAHEAD: usize>(&self) -> usize {
        debug_assert!(self.bits_left >= LOOKAHEAD as u8);
        (self.buffer & ((1 << LOOKAHEAD) - 1)) as usize
    }
    #[inline(always)]
    pub fn peek_var_bits(&self, lookahead: usize) -> usize {
        debug_assert!(self.bits_left >= lookahead as u8);
        (self.buffer & ((1 << lookahead) - 1)) as usize
    }

    #[inline(always)]
    pub fn get_bits(&mut self, num_bits: u8) -> u64 {
        debug_assert!(self.bits_left >= num_bits);

        let mask = (1_u64 << num_bits) - 1;

        let value = self.buffer & mask;

        self.buffer >>= num_bits;

        self.bits_left -= num_bits;

        value
    }
    /// Get number of bits left in the bit buffer.
    pub const fn get_bits_left(&self) -> u8 {
        self.bits_left
    }
    /// Get position the stream is in this buffer
    /// Or alternatively, number of bits read.
    pub fn get_position(&self) -> usize {
        self.position
            .saturating_sub(usize::from(self.bits_left >> 3))
    }

    /// Reset buffer and bits left to zero.
    pub fn reset(&mut self) {
        self.buffer = 0;
        self.bits_left = 0;
    }
    /// Return true if the bit buffer can satisfy
    /// `bits` read without refilling,
    pub const fn has(&self, bits: u8) -> bool {
        self.bits_left >= bits
    }

    #[inline(always)]
    pub fn drop_bits(&mut self, bits: u8) {
        debug_assert!(self.bits_left >= bits);
        self.bits_left -= bits;
        self.buffer >>= bits;
    }
    /// Return the remaining bytes in this stream.
    ///
    /// This does not consider bits in the bit-buffer hence
    /// may not be accurate
    pub const fn remaining_bytes(&self) -> usize {
        self.src.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_read_single_chunk() {
        // 0xAA = 10101010, 0x55 = 01010101
        let data = [0xAA, 0x55];
        let mut reader = BitStreamReader::new(&data, true);

        reader.refill();

        // Deflate reads LSB first.
        // 0xAA -> lower 4 bits are 1010 (10), upper are 1010 (10)
        assert_eq!(reader.get_bits(4), 0x0A);
        assert_eq!(reader.get_bits(4), 0x0A);

        // 0x55 -> lower 4 bits are 0101 (5), upper are 0101 (5)
        assert_eq!(reader.get_bits(4), 0x05);
        assert_eq!(reader.get_bits(4), 0x05);
    }

    #[test]
    fn test_non_final_chunk_starvation() {
        // Only 1 byte, and it's NOT the final chunk.
        let data = [0xFF];
        let mut reader = BitStreamReader::new(&data, false);

        reader.refill();

        // Since it's not final, refill_slow should NOT pad with zeros.
        // It should only load the exactly 8 bits available.
        assert_eq!(reader.get_bits_left(), 8);
        assert_eq!(reader.over_read, 0);

        // Read the 8 bits
        assert_eq!(reader.get_bits(8), 0xFF);

        // Try to refill again. It should do nothing because there's no data.
        reader.refill();
        assert_eq!(reader.get_bits_left(), 0);
        assert_eq!(reader.over_read, 0);
    }

    #[test]
    fn test_final_chunk_padding() {
        // Only 1 byte, but it IS the final chunk.
        let data = [0xFF];
        let mut reader = BitStreamReader::new(&data, true);

        reader.refill();

        // Because it's the final chunk, refill_slow SHOULD pad to at least 56 bits.
        assert!(reader.get_bits_left() >= 56);
        assert!(reader.over_read > 0);

        // The actual byte should be preserved at the bottom of the buffer
        assert_eq!(reader.get_bits(8), 0xFF);

        // The padded bits should be zeros
        assert_eq!(reader.get_bits(8), 0x00);
    }

    #[test]
    fn test_resume_across_chunk_boundary() {
        // We want to simulate reading a 12-bit value that is split across two IDAT chunks.
        // Chunk 1: 0xDD (1101_1101)
        // Chunk 2: 0xEE (1110_1110)

        let chunk1 = [0xDD];
        let chunk2 = [0xEE];

        let mut reader = BitStreamReader::new(&chunk1, false);
        reader.refill();

        // Read 4 bits from Chunk 1 (lower nibble of 0xDD is 1101 = 13 = 0x0D)
        assert_eq!(reader.get_bits(4), 0x0D);

        // Reader is now starved (4 bits left in buffer, but we want 8 more).
        assert_eq!(reader.get_bits_left(), 4);
        assert!(!reader.has(8));

        // SIMULATE PARSER FEEDING THE NEXT CHUNK
        reader.update_chunk(&chunk2, true);

        // Refill combines the remaining 4 bits from chunk1 with chunk2
        reader.refill();

        // The buffer should now hold the remaining 4 bits of chunk1 (1101 = 0x0D)
        // as the LSBs, followed by chunk2 (0xEE)
        // Let's read 8 bits. It should pull the 4 from chunk1, then the lower 4 from chunk2 (1110 = 0x0E).
        // Resulting byte: (0x0E << 4) | 0x0D = 0xED
        assert_eq!(reader.get_bits(8), 0xED);

        // Read the remaining 4 bits of chunk2
        assert_eq!(reader.get_bits(4), 0x0E);
    }

    #[test]
    fn test_peek_and_drop_across_chunks() {
        let chunk1 = [0x12];
        let mut reader = BitStreamReader::new(&chunk1, false);
        reader.refill();

        // We peek 8 bits, ensure it's correct, but drop only 4
        assert_eq!(reader.peek_bits::<8>(), 0x12);
        reader.drop_bits(4); // Drops the 2 (0010), leaving the 1 (0001)

        assert_eq!(reader.get_bits_left(), 4);

        // Feed next chunk
        let chunk2 = [0x34];
        reader.update_chunk(&chunk2, false);
        reader.refill();

        // Buffer has: 0x01 (from chunk 1) then 0x34 (from chunk 2)
        // Let's peek 12 bits.
        // 0x34 shifted by 4 is 0x340. OR with 0x01 -> 0x341.
        assert_eq!(reader.peek_var_bits(12), 0x341);

        // Consume them
        assert_eq!(reader.get_bits(12), 0x341);
        assert_eq!(reader.get_bits_left(), 0);
    }
}
