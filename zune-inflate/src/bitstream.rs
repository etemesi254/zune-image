//! `BitStreamReader` API
//!
//! This module provides an interface to read and write bits (and bytes) for
//! huffman

pub struct BitStreamReader<'src>
{
    // buffer from which we are pulling in bits from
    // used in decompression.
    src:      &'src [u8],
    // position in our buffer,
    position: usize,

    pub bits_left: u8,
    pub buffer:    u64
}

impl<'src> BitStreamReader<'src>
{
    /// Create a new `BitStreamReader` instance
    ///
    /// # Expectations
    /// The buffer must be padded with fill bytes in the end,
    /// if not, this becomes UB in the refill phase.
    pub fn new(in_buffer: &'src [u8]) -> BitStreamReader<'src>
    {
        BitStreamReader {
            bits_left: 0,
            buffer:    0,
            src:       in_buffer,
            position:  0
        }
    }
    /// Refill the bitstream ensuring the buffer has bits between
    /// 56 and 63.
    ///
    #[inline(always)]
    pub fn refill(&mut self)
    {
        /*
         * The refill always guarantees refills between 56-63
         *
         * Bits stored will never go above 63 and if bits are in the range 56-63 no refills occur.
         */
        let mut buf = [0; 8];

        match self.src.get(self.position..self.position + 8)
        {
            Some(bytes) =>
            {
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
            None => self.refill_slow()
        }
    }
    #[inline(never)]
    fn refill_slow(&mut self)
    {
        // ensure we don't read if we can do it in the
        // first byte
        assert!(self.position + 8 > self.src.len());
        let bytes = &self.src[self.position..];

        for byte in bytes
        {
            if self.bits_left >= 56
            {
                break;
            }

            self.buffer |= u64::from(*byte) << self.bits_left;
            self.bits_left += 8;
            self.position += 1;
        }
    }

    #[inline(always)]
    pub const fn peek_bits<const LOOKAHEAD: usize>(&self) -> usize
    {
        (self.buffer & ((1 << LOOKAHEAD) - 1)) as usize
    }
    #[inline(always)]
    pub fn peek_var_bits(&self, lookahead: usize) -> usize
    {
        (self.buffer & ((1 << lookahead) - 1)) as usize
    }

    #[inline(always)]
    pub fn get_bits(&mut self, num_bits: u8) -> u64
    {
        debug_assert!(self.bits_left >= num_bits);

        let mask = (1_u64 << num_bits) - 1;

        let value = self.buffer & mask;

        self.buffer >>= num_bits;

        self.bits_left -= num_bits;

        value
    }
    /// Get number of bits left in the bit buffer.
    pub const fn get_bits_left(&self) -> u8
    {
        self.bits_left
    }
    /// Get position the stream is in this buffer
    /// Or alternatively, number of bits read.
    pub const fn get_position(&self) -> usize
    {
        self.position
    }

    /// Reset buffer and bits left to zero.
    pub fn reset(&mut self)
    {
        self.buffer = 0;
        self.bits_left = 0;
    }

    /// Advance the reader n bytes ahead
    pub fn advance(&mut self, bytes: usize)
    {
        self.position = self.position.saturating_add(bytes);
    }
    /// Return true if the bit buffer can satisfy
    /// `bits` read without refilling,
    pub const fn has(&self, bits: u8) -> bool
    {
        self.bits_left >= bits
    }

    #[inline(always)]
    pub fn drop_bits(&mut self, bits: u8)
    {
        debug_assert!(self.bits_left >= bits);
        self.bits_left -= bits;
        self.buffer >>= bits;
    }
    /// Return the remaining bytes in this stream.
    ///
    /// This does not consider bits in the bit-buffer hence
    /// may not be accurate
    pub const fn remaining_bytes(&self) -> usize
    {
        self.src.len().saturating_sub(self.position)
    }
}
