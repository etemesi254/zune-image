// ============================================================
//  HEVC Bitstream Reader — MSB-first, 64-bit barrel buffer
// ============================================================
#![allow(dead_code)]
use crate::hevc_decoder::nal_parser::NalError;

pub struct BitReader<'src> {
    src:        &'src [u8],
    position:   usize,
    bits_left:  u8,
    buffer:     u64,
    // Track consecutive zeros seen so far (0, 1, or 2)
    zero_count: u8
}

impl<'src> BitReader<'src> {
    #[inline]
    pub fn new(src: &'src [u8]) -> Self {
        Self {
            src,
            position: 0,
            bits_left: 0,
            buffer: 0,
            zero_count: 0
        }
    }

    // ── Refill ────────────────────────────────────────────────────────────

    #[inline(always)]
    pub fn refill(&mut self) {
        if self.bits_left > 56 {
            return;
        }

        if let Some(bytes) = self.src.get(self.position..self.position + 4) {
            let chunk = u32::from_be_bytes(bytes.try_into().unwrap());

            // 1. Check if the chunk itself has 0x03
            // 2. CRITICAL: Check if the previous state (zero_count) combined with
            //    the start of this chunk creates a 00 00 03 sequence.
            let first_byte = (chunk >> 24) as u8;
            let creates_boundary_epb = self.zero_count == 2 && first_byte == 0x03;

            if !Self::has_emulation_prevention(chunk) && !creates_boundary_epb {
                self.load_4_bytes(chunk);
                return;
            }
        }

        self.refill_one_byte_at_a_time();
    }
    #[inline(always)]
    fn load_4_bytes(&mut self, chunk: u32) {
        // We are appending 32 bits to the buffer.
        // In a left-aligned buffer, the 'empty' space starts at self.bits_left.
        // We shift the 32-bit chunk so its MSB aligns with the next available slot.

        let shift = 64 - 32 - self.bits_left;
        self.buffer |= u64::from(chunk) << shift;

        self.bits_left += 32;
        self.position += 4;

        // After loading a chunk, we must update the zero_count state
        // based on the last two bytes of that chunk so the NEXT refill
        // knows if it's starting mid-sequence.
        self.update_zero_count_from_chunk(chunk);
    }

    #[inline(always)]
    fn update_zero_count_from_chunk(&mut self, chunk: u32) {
        // Check the last two bytes of the 32-bit word (Big Endian)
        let byte3 = (chunk & 0xFF) as u8;
        let byte2 = ((chunk >> 8) & 0xFF) as u8;

        if byte3 == 0x00 {
            if byte2 == 0x00 {
                self.zero_count = 2;
            } else {
                self.zero_count = 1;
            }
        } else {
            self.zero_count = 0;
        }
    }

    #[inline(always)]
    fn has_emulation_prevention( chunk: u32) -> bool {
        // SWAR check for 0x03 bytes.
        // This is a heuristic; if true, we go to the slow path.
        let m = chunk ^ 0x0303_0303;
        ((m.wrapping_sub(0x0101_0101)) & !m & 0x8080_8080) != 0
    }

    fn refill_one_byte_at_a_time(&mut self) {
        while self.bits_left <= 56 && self.position < self.src.len() {
            let mut byte = self.src[self.position];

            // HEVC Emulation Prevention: 00 00 03 -> 00 00
            if self.zero_count == 2 && byte == 0x03 {
                self.position += 1;
                self.zero_count = 0; // Reset after skipping
                if self.position >= self.src.len() {
                    break;
                }
                byte = self.src[self.position];
            }

            // Standard MSB buffer append
            self.buffer |= (u64::from(byte)) << (56 - self.bits_left);
            self.bits_left += 8;
            self.position += 1;

            // Update zero state
            if byte == 0x00 {
                self.zero_count = (self.zero_count + 1).min(2);
            } else {
                self.zero_count = 0;
            }
        }
    }

    // ── Peek ─────────────────────────────────────────────────────────────

    /// Return the next `N` bits right-aligned without consuming them.
    #[inline(always)]
    pub fn peek_bits<const N: u8>(&self) -> u64 {
        debug_assert!(N > 0 && N <= 56);
        debug_assert!(self.bits_left >= N);
        self.buffer >> (64 - u64::from(N))
    }

    // ── Consume ───────────────────────────────────────────────────────────

    #[inline(always)]
    pub fn drop_bits(&mut self, n: u8) {
        debug_assert!(self.bits_left >= n);
        self.buffer <<= n;
        self.bits_left -= n;
    }
    pub fn skip_bits(&mut self, n: u8) {
        let _ = self.get_bits(n);
    }

    // ── Read ──────────────────────────────────────────────────────────────

    /// Read and consume `n` bits, returned right-aligned in a u64.
    #[inline(always)]
    pub fn get_bits(&mut self, n: u8) -> Result<u64, &'static str> {
        debug_assert!(n > 0 && n <= 56);
        if self.bits_left < n {
            self.refill();
        }

        if self.bits_left < n {
            return Err("BitReader exhausted");
        }

        let shift = 64_u64.saturating_sub(u64::from(n));
        let val = self.buffer >> shift;
        self.buffer <<= n;
        self.bits_left = self.bits_left.saturating_sub(n);
        Ok(val)
    }

    /// Read a single bit as a bool.
    #[inline(always)]
    pub fn read_flag(&mut self) -> Result<bool, &'static str> {
        if self.bits_left < 1 {
            self.refill();
        }
        if self.bits_left < 1 {
            return Err("Bit reader exhausted");
        }
        let v = self.buffer >> 63;
        self.buffer <<= 1;
        self.bits_left -= 1;
        Ok(v == 1)
    }

    // ── HEVC Exp-Golomb ───────────────────────────────────────────────────

    /// Decode one unsigned Exp-Golomb codeword `ue(v)`.
    ///
    ///
    #[inline(always)]
    pub fn read_ue(&mut self) -> Result<u64, &'static str> {
        if self.bits_left < 32 {
            self.refill();
        }

        let num_zeros = self.buffer.leading_zeros() as u8;
        // HEVC (ITU-T H.265, E.3.3) allows ue(v) values up to 2^32-2 (e.g. bit_rate_value_minus1),
        // which requires 31 leading zeros in the exp-Golomb code. 32 leading zeros would give a
        // minimum codeNum of 2^32-1, which exceeds every syntax element's valid range.
        if num_zeros > 31 {
            return Err("ue num zeros code Num greater than 31 (max allowed)");
        }

        // Consume prefix zeros + stop bit.
        self.buffer <<= num_zeros + 1;
        self.bits_left -= num_zeros + 1;

        if num_zeros == 0 {
            return Ok(0);
        }
        if self.bits_left < 32 {
            self.refill();
        }

        let suffix = self.buffer >> (64 - u64::from(num_zeros));
        self.buffer <<= num_zeros;
        self.bits_left = self.bits_left.saturating_sub(num_zeros);

        Ok((1 << num_zeros) - 1 + suffix)
    }

    pub fn read_ue_u8(&mut self) -> Result<u8, NalError> {
        let val = self.read_ue()?;
        debug_assert!(val < u8::MAX.into());

        if val > u8::MAX.into() {
            let msg =
                format!("exp golomb decode for u8 failed, value {val} larger than u8::MAX(255)");
            return Err(NalError::Generic(msg));
        }
        return Ok(val as u8);
    }
    /// Decode one signed Exp-Golomb codeword `se(v)`.
    #[inline(always)]
    pub fn read_se(&mut self) -> Result<i64, &'static str> {
        let k = self.read_ue()?;
        Ok(match k {
            0 => 0,
            k if k & 1 == 1 => k.div_ceil(2).cast_signed(),
            k => -(k / 2).cast_signed()
        })
    }

    #[inline(always)]
    pub fn is_byte_aligned(&self) -> bool {
        self.bits_left.is_multiple_of(8)
    }

    pub fn byte_align(&mut self) -> Result<(), &'static str> {
        // Read the "1" bit and then all "0" bits until alignment
        let bit = self.read_flag()?;
        if bit {
            // Only align if we actually found the stop bit
            let rem = self.bits_left % 8;
            self.drop_bits(rem);
        }
        Ok(())
    }

    /// Returns the absolute bit position in the stream.
    /// This is the equivalent of HM's m_pcBitstream->getNumBitsRead().
    #[inline(always)]
    pub fn tell(&self) -> usize {
        // (Total bytes touched * 8) minus (bits still sitting in the buffer)
        (self.position * 8) - self.bits_left as usize
    }

    /// Returns the logical byte position.
    /// If we are mid-byte, this usually returns the start of that byte.
    #[inline(always)]
    pub fn byte_position(&self) -> usize {
        self.tell() / 8
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn padded(mut v: Vec<u8>) -> Vec<u8> {
        v.extend([0u8; 8]);
        v
    }

    #[test]
    fn nibbles() {
        let src = padded(vec![0xAB]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.get_bits(4).unwrap(), 0xA);
        assert_eq!(r.get_bits(4).unwrap(), 0xB);
    }

    #[test]
    fn two_full_bytes() {
        let src = padded(vec![0xFF, 0x00]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.get_bits(8), Ok(0xFF));
        assert_eq!(r.get_bits(8), Ok(0x00));
    }

    #[test]
    fn bits_spanning_byte_boundary() {
        // 0xAA 0xF0 → top 12 bits = 1010_1010_1111 = 0xAAF
        let src = padded(vec![0xAA, 0xF0]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.get_bits(12), Ok(0xAAF));
    }

    #[test]
    fn peek_does_not_consume() {
        let src = padded(vec![0xC0]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.peek_bits::<4>(), 0xC);
        assert_eq!(r.peek_bits::<4>(), 0xC);
        r.drop_bits(4);
        assert_eq!(r.get_bits(4), Ok(0x0));
    }

    #[test]
    fn flags() {
        let src = padded(vec![0b1010_0000]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_flag(), Ok(true));
        assert_eq!(r.read_flag(), Ok(false));
        assert_eq!(r.read_flag(), Ok(true));
        assert_eq!(r.read_flag(), Ok(false));
    }

    #[test]
    fn ue_zero() {
        let src = padded(vec![0b1000_0000]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_ue(), Ok(0));
    }

    #[test]
    fn ue_one() {
        let src = padded(vec![0b0100_0000]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_ue(), Ok(1));
    }

    #[test]
    fn ue_two() {
        let src = padded(vec![0b0110_0000]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_ue(), Ok(2));
    }

    #[test]
    fn ue_four() {
        let src = padded(vec![0b0010_1000]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_ue(), Ok(4));
    }

    #[test]
    fn ue_seven() {
        let src = padded(vec![0b0001_0000]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_ue(), Ok(7));
    }

    #[test]
    fn ue_sequential() {
        // "1"(0) ++ "010"(1) ++ "011"(2) → 0b1010_0110
        let src = padded(vec![0b1010_0110]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_ue(), Ok(0));
        assert_eq!(r.read_ue(), Ok(1));
        assert_eq!(r.read_ue(), Ok(2));
    }

    #[test]
    fn se_pos_and_neg() {
        // ue=1→+1, ue=2→-1  packed: "010"+"011" = 0b0100_1100
        let src = padded(vec![0b0100_1100]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_se(), Ok(1));
        assert_eq!(r.read_se(), Ok(-1));
    }

    #[test]
    fn se_zero() {
        let src = padded(vec![0b1000_0000]);
        let mut r = BitReader::new(&src);
        r.refill();
        assert_eq!(r.read_se(), Ok(0));
    }

    #[test]
    fn byte_align_drops_remainder() {
        let src = padded(vec![0xFF]);
        let mut r = BitReader::new(&src);
        r.refill();
        r.get_bits(3).unwrap();
        r.byte_align().unwrap();
        assert!(r.is_byte_aligned());
    }
}
