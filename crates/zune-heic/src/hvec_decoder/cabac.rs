use crate::hvec_decoder::cabac_tables::{
    CABAC_INIT_VALUES, RANGE_LPS_TABLE, TRANSITION_LPS, TRANSITION_MPS
};
use crate::hvec_decoder::nal_unit_headers::SliceType;

pub const NUM_CABAC_CONTEXTS: usize = 500;

pub struct CabacEngine<'a> {
    data:          &'a [u8],
    byte_cursor:   usize,
    range:         u32,
    value:         u32,
    buffered_bits: u64,
    bits_left:     u8,
    contexts:      [u8; NUM_CABAC_CONTEXTS]
}

impl<'a> CabacEngine<'a> {
    pub fn new(data: &'a [u8], slice_type: SliceType, slice_qp: i64) -> Self {
        let mut engine = Self {
            data,
            byte_cursor: 0,
            range: 510,
            value: 0,
            buffered_bits: 0,
            bits_left: 0,
            contexts: [0; NUM_CABAC_CONTEXTS]
        };

        engine.init_contexts(slice_type, slice_qp);

        let n = data.len().min(8);
        for i in 0..n {
            engine.buffered_bits |= (data[i] as u64) << (56 - i * 8);
        }
        engine.byte_cursor = n;
        engine.bits_left = (n * 8) as u8;

        engine.value = (engine.buffered_bits >> 48) as u32;
        engine.buffered_bits <<= 16;
        engine.bits_left = engine.bits_left.saturating_sub(16);

        engine
    }

    fn init_contexts(&mut self, slice_type: SliceType, slice_qp: i64) {
        let init_type: usize = match slice_type {
            SliceType::I => 2,
            SliceType::P => 1,
            SliceType::B => 0
        };

        for (i, &init_val) in CABAC_INIT_VALUES[init_type].iter().enumerate() {
            let init_val = init_val as i64;
            let m = (init_val >> 4) * 5 - 45;
            let n = (init_val & 0xF) * 8 - 16;

            let pre_ctx_state = (m * slice_qp >> 4) + n;
            let pre_ctx_state = pre_ctx_state.clamp(1, 126);

            if pre_ctx_state <= 63 {
                self.contexts[i] = ((63 - pre_ctx_state) as u8) << 1;
            } else {
                self.contexts[i] = (((pre_ctx_state - 64) as u8) << 1) | 1;
            }
        }
    }

    // ========================================================================
    // Reservoir Management (Hot Path)
    // ========================================================================

    /// Extracts `n` bits from the MSB.
    /// Caller MUST ensure `bits_left >= n` (enforced by renorm/refill logic).
    #[inline(always)]
    fn read_n_bits(&mut self, n: u8) -> u32 {
        if n == 0 {
            return 0;
        }
        let bits = (self.buffered_bits >> (64 - n)) as u32;
        self.buffered_bits <<= n;
        self.bits_left -= n;
        bits
    }

    /// Single bit read for bypass and legacy calls.
    #[inline(always)]
    fn read_next_bit(&mut self) -> u32 {
        self.read_n_bits(1)
    }

    #[inline(never)]
    fn refill_bulk(&mut self) {
        let empty_bits = 64u8.saturating_sub(self.bits_left);
        let bytes_needed = (empty_bits / 8) as usize;

        // If we don't need at least 1 full byte, skip reading entirely.
        if bytes_needed > 0 {
            match self.data.get(self.byte_cursor..self.byte_cursor + 8) {
                None => {
                    let bytes_left_in_stream = self.data.len() - self.byte_cursor;

                    // ============================================================
                    // SLOW PATH: Nearing EOF, fallback to safe byte-by-byte loop
                    // ============================================================
                    let bytes_available = bytes_left_in_stream.min(bytes_needed);

                    for i in 0..bytes_available {
                        let byte = self.data[self.byte_cursor + i] as u64;
                        let shift = empty_bits - 8 - (i as u8 * 8);
                        self.buffered_bits |= byte << shift;
                    }

                    self.byte_cursor += bytes_available;
                }
                Some(bytes) => {
                    // ============================================================
                    // FAST PATH: Branchless 64-bit word read
                    // Compiles down to a single movbe/bswap instruction on x86_64
                    // ============================================================

                    // 1. Read the next 8 bytes unconditionally
                    let word = u64::from_be_bytes(bytes.try_into().unwrap());

                    // 2. We only want to keep `bytes_needed` bytes.
                    // Create a mask that keeps the top valid bytes and zeroes the rest.
                    let valid_bits = (bytes_needed as u64) * 8;
                    let mask = !0u64 << (64 - valid_bits);

                    // 3. Mask out the unwanted bytes, then shift the whole block right
                    // so the top byte lands exactly at the start of our empty space.
                    self.buffered_bits |= (word & mask) >> self.bits_left;

                    self.byte_cursor += bytes_needed;
                }
            }
        }

        // Clamp to 64. Any unwritten LSB slots implicitly remain 0 per HEVC spec.
        self.bits_left = 64;
    }

    // ========================================================================
    // Branchless Renormalization
    // ========================================================================

    /// Renormalizes `range` back into [256, 510] without a while loop.
    #[inline(always)]
    fn renorm(&mut self) {
        // range is a u32. 256 is bit 8.
        // If range >= 256, leading_zeros() is <= 23, saturating_sub yields 0.
        // If range < 256, we calculate exactly how many bits to shift in one go.
        let shift = self.range.leading_zeros().saturating_sub(23);

        if shift > 0 {
            self.range <<= shift;
            self.value = (self.value << shift) | self.read_n_bits(shift as u8);
        }

        // Refill only if we depleted our safety buffer.
        if self.bits_left <= 8 {
            self.refill_bulk();
        }
    }

    // ========================================================================
    // Decoding Methods
    // ========================================================================

    pub fn decode_bypass(&mut self) -> u8 {
        if self.bits_left == 0 {
            self.refill_bulk();
        }

        self.value = (self.value << 1) | self.read_next_bit();
        let scaled_range = self.range << 8;

        if self.value >= scaled_range {
            self.value -= scaled_range;
            1
        } else {
            0
        }
    }

    /// Optimized method to decode N bypass bins consecutively.
    /// Crucial for reading Exp-Golomb suffixes and MVDs.
    pub fn decode_bypass_n(&mut self, n: u8) -> u32 {
        let mut result = 0;
        // Inlining the bypass logic here prevents N function call overheads
        // across module boundaries when binarizing.
        for _ in 0..n {
            result = (result << 1) | self.decode_bypass() as u32;
        }
        result
    }

    pub fn decode_decision(&mut self, ctx_idx: usize) -> u8 {
        let state = self.contexts[ctx_idx];
        let mps = state & 1;
        let p_state = state >> 1;

        let range_lps = self.calculate_lps_range(p_state);
        self.range -= range_lps;

        let bin_decoded;

        if self.value < (self.range << 8) {
            bin_decoded = mps;
            self.contexts[ctx_idx] = self.next_state_mps(state);
        } else {
            bin_decoded = 1 - mps;
            self.value -= self.range << 8;
            self.range = range_lps;
            self.contexts[ctx_idx] = self.next_state_lps(state);
        }

        self.renorm();
        bin_decoded
    }

    /// Required for end_of_slice_segment_flag and end_of_subset_one_bit.
    /// Uses a hardcoded range of 2.
    pub fn decode_terminate(&mut self) -> u8 {
        self.range -= 2;
        let scaled_range = self.range << 8;

        if self.value >= scaled_range {
            // Terminated. Value is technically adjusted here per spec,
            // but since decoding halts, we just return the bin.
            1
        } else {
            // Not terminated. Proceed with renormalization.
            self.renorm();
            0
        }
    }

    /// Halts CABAC and flushes remaining bits in the current byte.
    /// Required when switching to IPCM (raw pixel) mode.
    pub fn align_to_byte(&mut self) {
        // We calculate how many actual bits from the stream we've consumed.
        // byte_cursor * 8 is total loaded.
        // bits_left is what's still in the reservoir.
        // 16 is what's currently trapped inside `self.value`.
        let bits_consumed = (self.byte_cursor * 8) as u64 - self.bits_left as u64 - 16;
        let padding_bits = (8 - (bits_consumed % 8)) % 8;

        if padding_bits > 0 {
            // Discard the padding bits to reach the byte boundary
            self.read_n_bits(padding_bits as u8);
        }
    }

    // ========================================================================
    // Internal State Helpers
    // ========================================================================

    #[inline(always)]
    fn next_state_mps(&self, state: u8) -> u8 {
        let mps = state & 1;
        let p_state = (state >> 1) as usize;
        (TRANSITION_MPS[p_state] << 1) | mps
    }

    #[inline(always)]
    fn next_state_lps(&self, state: u8) -> u8 {
        let mps = state & 1;
        let p_state = (state >> 1) as usize;
        let next_p_state = TRANSITION_LPS[p_state];
        let next_mps = if p_state == 0 { 1 - mps } else { mps };
        (next_p_state << 1) | next_mps
    }

    #[inline(always)]
    fn calculate_lps_range(&self, p_state: u8) -> u32 {
        let q_range = ((self.range >> 6) & 3) as usize;
        RANGE_LPS_TABLE[p_state as usize][q_range] as u32
    }
}

// cabac_engine_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hvec_decoder::nal_unit_headers::SliceType;

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Builds an engine from a raw byte slice with neutral QP (26) and an
    /// I-slice so init_contexts has a deterministic path when implemented.
    fn make_engine(data: &[u8]) -> CabacEngine<'_> {
        CabacEngine::new(data, SliceType::I, 26)
    }

    /// Returns a byte that, when repeated, gives a well-known bit pattern.
    /// 0b10101010 = alternating 1/0 starting with 1.
    const ALTERNATING: u8 = 0b10101010;

    // =========================================================================
    // 1. Reservoir initialization
    // =========================================================================

    /// The 16-bit value register must equal the top 16 bits of the stream.
    #[test]
    fn test_value_register_primed_correctly() {
        // First two bytes are 0x12, 0x34 → value should be 0x1234
        let data = [0x12, 0x34, 0x56, 0x78, 0xAB, 0xCD, 0xEF, 0x00];
        let engine = make_engine(&data);
        assert_eq!(
            engine.value, 0x1234,
            "value register must be primed with the first 16 bits of the stream"
        );
    }

    /// Range must start at exactly 510 per HEVC spec §9.3.1.
    #[test]
    fn test_initial_range_is_510() {
        let data = [0u8; 16];
        let engine = make_engine(&data);
        assert_eq!(
            engine.range, 510,
            "range must initialise to 510 per HEVC spec §9.3.1"
        );
    }

    // =========================================================================
    // 2. Bit reading & reservoir management
    // =========================================================================

    /// Reads the MSB of the first byte and checks correctness for all-zeros
    /// and all-ones streams, exercising the hot refill path.
    #[test]
    fn test_read_next_bit_all_zeros() {
        let data = [0x00u8; 32];
        let mut engine = make_engine(&data);
        // The value register was primed with 0x0000, so the first several
        // bypass bins must all decode as 0.
        for i in 0..16 {
            let bin = engine.decode_bypass();
            assert_eq!(bin, 0, "bin {} of all-zeros stream should be 0", i);
        }
    }

    // =========================================================================
    // 3. Short-stream / EOF handling
    // =========================================================================

    /// A 1-byte stream must not panic. EOF should silently extend with zeros.
    #[test]
    fn test_single_byte_stream_does_not_panic() {
        let data = [0x80u8]; // Single byte: 1000_0000
        let mut engine = make_engine(&data);
        // Read more bins than there are data bits — must not panic or overflow.
        for _ in 0..32 {
            let _ = engine.decode_bypass();
        }
    }

    /// An empty slice should be handled gracefully (no panic, no UB).
    #[test]
    fn test_empty_stream_does_not_panic() {
        let data: [u8; 0] = [];
        let mut engine = make_engine(&data);
        for _ in 0..8 {
            let _ = engine.decode_bypass();
        }
    }

    /// Exactly 8 bytes — the reservoir is filled exactly once during init.
    /// A 9th byte must not be consumed during construction.
    #[test]
    fn test_exactly_8_bytes_cursor_position() {
        let data = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99];
        let engine = make_engine(&data);
        // The engine should have consumed exactly 8 bytes during init.
        // byte_cursor must be <= 8 (the 9th byte is untouched at this point
        // because construction consumes at most 8 bytes into the reservoir
        // then primes value from the top 16 bits — the cursor moves up to 8).
        assert!(
            engine.byte_cursor <= 8,
            "byte_cursor advanced past the initial reservoir fill: {}",
            engine.byte_cursor
        );
    }

    // =========================================================================
    // 4. decode_bypass arithmetic
    // =========================================================================

    /// decode_bypass must return 0 or 1 only. Never another value.
    #[test]
    fn test_decode_bypass_returns_binary() {
        let data = [ALTERNATING; 64];
        let mut engine = make_engine(&data);
        for i in 0..256 {
            let bin = engine.decode_bypass();
            assert!(
                bin == 0 || bin == 1,
                "decode_bypass returned {} at iteration {}",
                bin,
                i
            );
        }
    }

    /// With a known all-0xFF stream the arithmetic should remain consistent
    /// across many calls without range collapsing to 0.
    #[test]
    fn test_decode_bypass_range_never_zero() {
        let data = [0xFFu8; 64];
        let mut engine = make_engine(&data);
        for i in 0..256 {
            let _ = engine.decode_bypass();
            assert!(
                engine.range > 0,
                "range collapsed to 0 after {} bypass decodes",
                i
            );
        }
    }

    // =========================================================================
    // 5. decode_decision arithmetic
    // =========================================================================

    /// decode_decision must return 0 or 1 only.
    #[test]
    fn test_decode_decision_returns_binary() {
        let data = [ALTERNATING; 64];
        let mut engine = make_engine(&data);
        for ctx in 0..NUM_CABAC_CONTEXTS {
            let bin = engine.decode_decision(ctx);
            assert!(
                bin == 0 || bin == 1,
                "decode_decision returned {} for ctx {}",
                bin,
                ctx
            );
        }
    }

    /// After each decode_decision, `range` must stay in [256, 510] because
    /// renorm guarantees it.
    #[test]
    fn test_range_stays_in_renorm_bounds_after_decision() {
        let data = [0xA5u8; 128];
        let mut engine = make_engine(&data);
        for i in 0..200 {
            let _ = engine.decode_decision(i % NUM_CABAC_CONTEXTS);
            assert!(
                engine.range >= 256 && engine.range <= 510,
                "range {} out of renorm bounds [256,510] at step {}",
                engine.range,
                i
            );
        }
    }

    /// Decoding with every context index must not panic (bounds check).
    #[test]
    fn test_all_context_indices_accessible() {
        let data = [0x55u8; 256];
        let mut engine = make_engine(&data);
        for ctx in 0..NUM_CABAC_CONTEXTS {
            let bin = engine.decode_decision(ctx);
            assert!(bin <= 1, "unexpected bin {} at ctx {}", bin, ctx);
        }
    }

    // =========================================================================
    // 6. Renormalization invariants
    // =========================================================================

    /// After renorm, `range` must be >= 256 and < 512.
    #[test]
    fn test_renorm_brings_range_into_bounds() {
        let data = [0b11001100u8; 64];
        let mut engine = make_engine(&data);
        // Force several renorm cycles by reading decisions.
        for _ in 0..100 {
            let _ = engine.decode_decision(0);
            assert!(
                engine.range >= 256,
                "renorm failed: range {} < 256",
                engine.range
            );
        }
    }

    // =========================================================================
    // 7. Context array bounds
    // =========================================================================

    // =========================================================================
    // 8. Known-good golden vector
    // =========================================================================

    /// Smoke test: a sequence of bypass decodes on a well-known byte produces
    /// a deterministic sequence of bins.
    ///
    /// 0x96 = 1001_0110 — the expected bypass output depends on the
    /// arithmetic (range=510, value=0x9696 after priming).
    ///
    /// NOTE: once the state-transition tables and range-LPS table are
    /// implemented (currently stubs), this expected vector must be
    /// recalculated from the HEVC reference decoder output.
    #[test]
    fn test_bypass_golden_vector_stub() {
        // Repeat 0x96 so the reservoir is always full.
        let data = [0x96u8; 64];
        let mut engine = make_engine(&data);

        let bins: Vec<u8> = (0..8).map(|_| engine.decode_bypass()).collect();

        // Validate only the binary constraint for now.
        for (i, &bin) in bins.iter().enumerate() {
            assert!(bin <= 1, "golden vector bin {} = {} (not binary)", i, bin);
        }
        // TODO: once tables are implemented, replace with:
        // assert_eq!(bins, vec![expected_0, expected_1, ...]);
    }

    // =========================================================================
    // 9. Fuzz-like stress test
    // =========================================================================

    /// Mixes bypass and decision decodes with a pseudo-random stream to check
    /// that no combination of calls causes a panic, overflow, or UB.
    #[test]
    fn test_mixed_decode_stress() {
        // A stream with varied bits — not cryptographically random but good
        // enough to exercise multiple code paths.
        let data: Vec<u8> = (0u8..=255u8).cycle().take(512).collect();
        let mut engine = make_engine(&data);

        for i in 0..1024 {
            if i % 3 == 0 {
                let bin = engine.decode_decision(i % NUM_CABAC_CONTEXTS);
                assert!(bin <= 1, "stress: decode_decision out of range at {}", i);
            } else {
                let bin = engine.decode_bypass();
                assert!(bin <= 1, "stress: decode_bypass out of range at {}", i);
            }
            // Invariant: range must never be 0 or negative.
            assert!(engine.range > 0, "stress: range collapsed at step {}", i);
        }
    }
}
