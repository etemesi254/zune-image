use crate::debug_more;
use crate::hvec_decoder::DEBUG_MORE;
use crate::hvec_decoder::cabac_tables::*;

pub const NUM_CABAC_CONTEXTS: usize = 171;

// --- ENGINE IMPLEMENTATION ---

pub struct CabacDecoder<'a> {
    data:   &'a [u8],
    cursor: usize,

    pub range:       u32,
    pub value:       u32,
    pub bits_needed: i8,
    pub contexts:    [u8; NUM_CABAC_CONTEXTS]
}

impl<'a> CabacDecoder<'a> {
    pub fn new(data: &'a [u8], slice_qp: i32, init_type: usize) -> Self {
        let mut engine = Self {
            data,
            cursor: 0,
            range: 510,
            value: 0,
            bits_needed: -8,
            contexts: [0; NUM_CABAC_CONTEXTS]
        };

        engine.init_contexts(slice_qp, init_type);
        engine.init_value();
        engine
    }

    fn init_value(&mut self) {
        if self.data.len() >= 2 {
            self.value = (self.data[0] as u32) << 8 | (self.data[1] as u32);
            self.cursor = 2;
            self.bits_needed = -8;
            debug_more!(
                "init_CABAC_decode_2 range :{} value :{}",
                self.range,
                self.value
            );
        }
    }

    /// Batch renorm: shifts value/range by `shift` bits and reads at most one
    /// new byte. Since shift <= 6 in the LPS path and bits_needed starts at
    /// -8, a single byte always covers the demand — no loop needed.
    #[inline(always)]
    fn renorm(&mut self, shift: u32) {
        self.value <<= shift;
        self.bits_needed += shift as i8;

        if self.bits_needed >= 0 {
            // We have consumed enough bits that a new byte is needed.
            let byte = if self.cursor < self.data.len() {
                let b = self.data[self.cursor];
                self.cursor += 1;
                b as u32
            } else {
                0 // Trailing padding — safe to treat as zero.
            };
            // Place the new byte so its MSB lands at bit (7 - bits_needed).
            self.value |= byte << self.bits_needed;
            self.bits_needed -= 8;
        }
    }

    /// Single-bit renorm kept for the MPS path (shift == 1 always).
    #[inline(always)]
    fn renorm_one(&mut self) {
        self.renorm(1);
    }

    // --- Core Decoding Functions ---

    #[inline(always)]
    pub fn decode_decision(&mut self, ctx_idx: usize) -> u8 {
        let state_packed = self.contexts[ctx_idx];
        let mps = state_packed & 1;
        let state = (state_packed >> 1) as usize;

        debug_more!(
            "decodeBin range :{} value:{} state:{}",
            self.range,
            self.value,
            state
        );

        let q_idx = (self.range >> 6) & 3;
        let lps_range = RANGE_LPS_TABLE[state][q_idx as usize] as u32;

        self.range -= lps_range;
        let scaled_range = self.range << 7;

        debug_more!(" sr:{} v:{}", scaled_range, self.value);

        if self.value < scaled_range {
            // --- MPS path (hot ~94 %+ of the time) ---
            debug_more!(" MPS");
            self.contexts[ctx_idx] = (TRANSITION_MPS[state] << 1) | mps;

            if self.range < 256 {
                self.range <<= 1;
                self.renorm_one();
            }

            debug_more!(" -> bit {}  r:{} v:{}", mps, self.range, self.value);
            mps
        } else {
            // --- LPS path (cold) ---
            lps_decode(self, ctx_idx, mps, state, lps_range, scaled_range)
        }
    }

    #[inline(always)]
    pub fn decode_bypass(&mut self) -> u8 {
        debug_more!("bypass r:{} v:{}", self.range, self.value);
        self.renorm_one();

        let scaled_range = self.range << 7;
        let bit = (self.value >= scaled_range) as u8;
        if bit == 1 {
            self.value -= scaled_range;
        }

        debug_more!(" -> bit {}  r:{} v:{}", bit, self.range, self.value);
        bit
    }

    #[inline(always)]
    pub fn decode_terminate(&mut self) -> u8 {
        debug_more!("CABAC term: range={:x}", self.range);

        self.range -= 2;
        let scaled_range = self.range << 7;

        if self.value >= scaled_range {
            1
        } else {
            if self.range < 256 {
                self.range <<= 1;
                self.renorm_one();
            }
            0
        }
    }

    // --- Specialized bypass decoders ---

    /// Decode `n_bits` bypass bins in one pass.
    /// All renorms are done up front; then bits are peeled off the value
    /// register without touching the range register again.
    pub fn decode_fl_bypass(&mut self, n_bits: u8) -> u32 {
        // Renorm all bits at once.
        self.renorm(n_bits as u32);

        let scaled = self.range << 7;
        let mut res = 0u32;

        for i in (0..n_bits).rev() {
            if self.value >= scaled {
                self.value -= scaled;
                res |= 1 << i;
            }
            // Each iteration represents one bypass step; value was
            // pre-shifted by renorm so we don't need extra shifts here.
        }
        res
    }

    pub fn decode_bypass_eg0(&mut self) -> u32 {
        // Count leading 1-bins (prefix).
        let mut prefix = 0u32;
        while self.decode_bypass() == 1 {
            prefix += 1;
            // Guard against malformed streams (max EG0 prefix in practice ≤ 12).
            if prefix > 12 {
                break;
            }
        }

        if prefix == 0 {
            return 0;
        }

        // Read `prefix` suffix bits in one batched call.
        let suffix = self.decode_fl_bypass(prefix as u8);
        (1 << prefix) - 1 + suffix
    }

    /// Decodes Truncated Unary bypass values.
    pub fn decode_tu_bypass(&mut self, c_max: u8) -> u8 {
        for i in 0..c_max {
            if self.decode_bypass() == 0 {
                return i;
            }
        }
        c_max
    }
}

/// Cold LPS path extracted to its own function so the branch predictor and
/// inliner can treat the MPS path in `decode_decision` as the sole hot path.
#[cold]
#[inline(never)]
fn lps_decode(
    dec: &mut CabacDecoder<'_>, ctx_idx: usize, mps: u8, state: usize, lps_range: u32,
    scaled_range: u32
) -> u8 {
    let bin = 1 - mps;
    debug_more!(" LPS");
    dec.value -= scaled_range;

    let shift = RENORM_TABLE[(lps_range >> 3) as usize] as u32;
    dec.range = lps_range << shift;

    // Single batched renorm replaces the original `for _ in 0..shift` loop.
    dec.renorm(shift);

    let next_mps = if state == 0 { 1 - mps } else { mps };
    dec.contexts[ctx_idx] = (TRANSITION_LPS[state] << 1) | next_mps;

    debug_more!(" -> bit {}  r:{} v:{}", bin, dec.range, dec.value);
    bin
}

impl<'a> CabacDecoder<'a> {
    pub fn init_contexts(&mut self, qp: i32, init_type: usize) {
        let qp_y = qp.clamp(0, 51);

        // 1. Initialize Motion Contexts (Only for P/B slices)
        if init_type > 0 {
            // libde265 uses initType: 0=B, 1=P, 2=I
            self.set_init(
                qp_y,
                CONTEXT_MODEL_CU_SKIP_FLAG,
                &INIT_CU_SKIP[init_type],
                3
            );
            self.set_init(
                qp_y,
                CONTEXT_MODEL_PRED_MODE_FLAG,
                &[INIT_PRED_MODE[init_type]],
                1
            );
            self.set_init(
                qp_y,
                CONTEXT_MODEL_MERGE_FLAG,
                &[INIT_MERGE_FLAG[init_type]],
                1
            );
            self.set_init(
                qp_y,
                CONTEXT_MODEL_MERGE_IDX,
                &[INIT_MERGE_IDX[init_type]],
                1
            );
            self.set_init(qp_y, CONTEXT_MODEL_INTER_PRED_IDC, &INIT_INTER_PRED_IDC, 5);
            self.set_init(qp_y, CONTEXT_MODEL_REF_IDX_LX, &INIT_REF_IDX, 2);

            let mvd_idx = if init_type == 1 { 0 } else { 2 };
            self.set_init(
                qp_y,
                CONTEXT_MODEL_ABS_MVD_GREATER01_FLAG,
                &INIT_ABS_MVD[mvd_idx..],
                2
            );
            self.set_init(qp_y, CONTEXT_MODEL_MVP_LX_FLAG, &INIT_MVP_LX, 1);
            self.set_init(qp_y, CONTEXT_MODEL_RQT_ROOT_CBF, &INIT_RQT_ROOT, 1);

            // RDPCM (Constant 139)
            for i in 0..2 {
                self.set_init_const(qp_y, CONTEXT_MODEL_RDPCM_FLAG + i, 139);
                self.set_init_const(qp_y, CONTEXT_MODEL_RDPCM_DIR + i, 139);
            }
        }

        // 2. Initialize Common Contexts (All slices)
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SPLIT_CU_FLAG,
            &INIT_SPLIT_CU[init_type],
            3
        );

        let part_idx = if init_type != 2 { init_type } else { 5 };
        self.set_init(
            qp_y,
            CONTEXT_MODEL_PART_MODE,
            &INIT_PART_MODE[part_idx..],
            4
        );

        self.set_init(
            qp_y,
            CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG,
            &[INIT_PREV_INTRA[init_type]],
            1
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE,
            &[INIT_CHROMA_PRED[init_type]],
            1
        );

        let cbf_l_idx = if init_type == 0 { 0 } else { 2 };
        self.set_init(qp_y, CONTEXT_MODEL_CBF_LUMA, &INIT_CBF_LUMA[cbf_l_idx..], 2);
        self.set_init(
            qp_y,
            CONTEXT_MODEL_CBF_CHROMA,
            &INIT_CBF_CHROMA[init_type * 4..],
            4
        );

        self.set_init(
            qp_y,
            CONTEXT_MODEL_SPLIT_TRANSFORM_FLAG,
            &INIT_SPLIT_TRANS[init_type * 3..],
            3
        );

        // Residuals
        self.set_init(
            qp_y,
            CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX,
            &INIT_LAST_COEFF[init_type * 18..],
            18
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX,
            &INIT_LAST_COEFF[init_type * 18..],
            18
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG,
            &INIT_CODED_SUB[init_type * 4..],
            4
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG,
            &INIT_SIG_COEFF[init_type],
            42
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG + 42,
            &INIT_SIG_COEFF_SKIP[init_type],
            2
        );

        // SAO
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SAO_MERGE_FLAG,
            &[INIT_SAO_MERGE[init_type]],
            1
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SAO_TYPE_IDX,
            &[INIT_SAO_TYPE[init_type]],
            1
        );
    }

    fn set_init(&mut self, qp: i32, start_idx: usize, values: &[u8], len: usize) {
        // libde265 does Clip3(0, 51, SliceQPY)
        let qp_clipped = qp.clamp(0, 51);

        for i in 0..len {
            let iv = values[i] as i32;
            let slope_idx = iv >> 4;
            let intersec_idx = iv & 0xF;

            let m = slope_idx * 5 - 45;
            let n = (intersec_idx << 3) - 16;

            // Using arithmetic shift >> 4 on i32 is equivalent to C's signed shift
            let pre = ((m * qp_clipped) >> 4) + n;
            let pre = pre.clamp(1, 126);

            if pre <= 63 {
                // MPS = 0, State = 63 - pre
                // Packed as (state << 1) | mps
                self.contexts[start_idx + i] = ((63 - pre) as u8) << 1;
            } else {
                // MPS = 1, State = pre - 64
                // Packed as (state << 1) | mps
                self.contexts[start_idx + i] = (((pre - 64) as u8) << 1) | 1;
            }
        }
    }

    fn set_init_const(&mut self, qp: i32, idx: usize, iv: u8) {
        self.set_init(qp, idx, &[iv], 1);
    }
}
