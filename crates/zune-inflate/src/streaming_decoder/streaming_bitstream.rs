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

#[derive(Default)]
pub struct StreamingBitStreamReader {
    // position in our buffer,
    pub position: usize,
    pub bits_left: u8,
    pub buffer: u64,
    pub over_read: usize,
    // Indicates if this is the last chunk of the stream.
    pub is_final: bool,
}

impl StreamingBitStreamReader {
    /// Create a new empty `StreamingBitStream` instance
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_chunk(&mut self, is_final: bool) {
        self.is_final = is_final;
        // Wipe out any "Ghost Bytes" trapped in the buffer
        self.buffer &= (1_u64 << self.bits_left) - 1;
    }

    /// Refill the bitstream ensuring the buffer has bits between
    /// 56 and 63.
    #[inline(always)]
    pub fn refill(&mut self, src: &[u8]) {
        /*
         * The refill always guarantees refills between 56-63
         *
         * Bits stored will never go above 63 and if bits are in the range 56-63 no refills occur.
         */
        let mut buf = [0; 8];

        match src.get(self.position..self.position + 8) {
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
            None => self.refill_slow(src),
        }
    }

    #[inline(never)]
    fn refill_slow(&mut self, src: &[u8]) {
        let bytes = src.get(self.position..).unwrap_or(&[]);

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
            debug_assert!(self.over_read <= 8);
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
    pub fn reset_position(&mut self) {
        self.position = 0;
    }

    /// Get number of bits left in the bit buffer.
    pub const fn get_bits_left(&self) -> u8 {
        self.bits_left
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
    pub const fn remaining_bytes(&self, src: &[u8]) -> usize {
        src.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_read_single_chunk() {
        let data = [0xAA, 0x55];
        let mut reader = StreamingBitStreamReader::new();
        reader.update_chunk(true);
        reader.refill(&data);

        assert_eq!(reader.get_bits(4), 0x0A);
        assert_eq!(reader.get_bits(4), 0x0A);
        assert_eq!(reader.get_bits(4), 0x05);
        assert_eq!(reader.get_bits(4), 0x05);
    }

    #[test]
    fn test_non_final_chunk_starvation() {
        let data = [0xFF];
        let mut reader = StreamingBitStreamReader::new();
        reader.update_chunk(false);
        reader.refill(&data);

        assert_eq!(reader.get_bits_left(), 8);
        assert_eq!(reader.over_read, 0);
        assert_eq!(reader.get_bits(8), 0xFF);

        reader.refill(&data);
        assert_eq!(reader.get_bits_left(), 0);
        assert_eq!(reader.over_read, 0);
    }

    #[test]
    fn test_final_chunk_padding() {
        let data = [0xFF];
        let mut reader = StreamingBitStreamReader::new();
        reader.update_chunk( true);
        reader.refill(&data);

        assert!(reader.get_bits_left() >= 56);
        assert!(reader.over_read > 0);
        assert_eq!(reader.get_bits(8), 0xFF);
        assert_eq!(reader.get_bits(8), 0x00);
    }

    #[test]
    fn test_resume_across_chunk_boundary() {
        let chunk1 = [0xDD];
        let chunk2 = [0xEE];

        let mut reader = StreamingBitStreamReader::new();
        reader.update_chunk( false);
        reader.refill(&chunk1);

        assert_eq!(reader.get_bits(4), 0x0D);
        assert_eq!(reader.get_bits_left(), 4);
        assert!(!reader.has(8));

        reader.update_chunk( true);
        reader.refill(&chunk2);

        assert_eq!(reader.get_bits(8), 0xED);
        assert_eq!(reader.get_bits(4), 0x0E);
    }

    #[test]
    fn test_peek_and_drop_across_chunks() {
        let chunk1 = [0x12];
        let mut reader = StreamingBitStreamReader::new();
        reader.update_chunk( false);
        reader.refill(&chunk1);

        assert_eq!(reader.peek_bits::<8>(), 0x12);
        reader.drop_bits(4);
        assert_eq!(reader.get_bits_left(), 4);

        let chunk2 = [0x34];
        reader.update_chunk( false);
        reader.refill(&chunk2);

        assert_eq!(reader.peek_var_bits(12), 0x341);
        assert_eq!(reader.get_bits(12), 0x341);
        assert_eq!(reader.get_bits_left(), 0);
    }
}
