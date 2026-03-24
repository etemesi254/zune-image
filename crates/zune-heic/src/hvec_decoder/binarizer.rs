use crate::hvec_decoder::cabac::CabacEngine;

pub struct Binarizer<'a, 'b> {
    pub engine: &'a mut CabacEngine<'b>
}

impl<'a, 'b> Binarizer<'a, 'b> {
    pub fn new(engine: &'a mut CabacEngine<'b>) -> Self {
        Self { engine }
    }

    // ========================================================================
    // 1. Fixed-Length (FL) Binarization
    // ========================================================================

    /// HEVC Spec §9.3.3.4
    /// Reads exactly `length` bins. Used for things where the max value is
    /// a known power of 2 (e.g., chroma prediction modes, or raw payload bits).
    #[inline(always)]
    pub fn decode_fl(&mut self, length: u8) -> u32 {
        self.engine.decode_bypass_n(length)
    }

    // ========================================================================
    // 2. k-th Order Exp-Golomb (EGk) Binarization
    // ========================================================================

    /// HEVC Spec §9.3.3.3
    /// EGk is used for large values. It counts how many `1`s appear before
    /// hitting a `0` (the prefix), then reads a specific number of bypass
    /// bins (the suffix) based on that count and `k`.
    pub fn decode_egk(&mut self, k: u8) -> u32 {
        let mut leading_ones = 0;

        // Count 1s until we hit a 0
        while self.engine.decode_bypass() == 1 {
            leading_ones += 1;
        }

        // The length of the suffix grows linearly with the number of leading ones
        let suffix_len = leading_ones + k;
        let suffix = self.engine.decode_bypass_n(suffix_len);

        // Calculate the final integer value
        let value = (1 << leading_ones) - 1;
        (value << k) + suffix
    }

    // ========================================================================
    // 3. Truncated Rice (TR) Binarization
    // ========================================================================

    /// HEVC Spec §9.3.3.2
    /// This is the workhorse of HEVC. It is used millions of times per frame
    /// to decode `coeff_abs_level_remaining` (the actual pixel residual data).
    /// It uses a "Rice Parameter" (cRiceParam) that adapts dynamically.
    pub fn decode_tr(&mut self, c_rice_param: u8, c_max: u32) -> u32 {
        let mut prefix = 0;
        let prefix_max = c_max >> c_rice_param;

        // Read the Unary prefix (count 1s until a 0 is hit, bounded by prefix_max)
        while prefix < prefix_max && self.engine.decode_bypass() == 1 {
            prefix += 1;
        }

        if prefix < prefix_max {
            // Standard TR: We hit a 0 before maxing out.
            // The value is simply the prefix shifted, plus `c_rice_param` suffix bits.
            let suffix = self.engine.decode_bypass_n(c_rice_param);
            (prefix << c_rice_param) | suffix
        } else {
            // Escape TR: We maxed out the prefix!
            // HEVC handles giant numbers by appending an EGk code to the end,
            // where the EGk order `k` is `c_rice_param + 1`.
            let egk_val = self.decode_egk(c_rice_param + 1);
            (prefix << c_rice_param) + egk_val
        }
    }
}
