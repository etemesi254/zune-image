use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::*;

pub const NUM_CABAC_CONTEXTS: usize = 172;

// --- ENGINE IMPLEMENTATION ---

pub struct CabacDecoder<'a> {
    pub data:        &'a [u8],
    pub cursor:      usize,
    pub range:       u32,
    pub value:       u32,
    pub bits_needed: i8,
    // TODO: Investigate whether a stack one makes it faster
    pub contexts:    Vec<u8>
}

impl<'a> CabacDecoder<'a> {
    pub fn new(data: &'a [u8], slice_qp: i32, init_type: usize) -> Self {
        let mut engine = Self {
            data,
            cursor: 0,
            range: 510,
            value: 0,
            bits_needed: -8,
            contexts: vec![0; NUM_CABAC_CONTEXTS]
        };

        engine.init_contexts(slice_qp, init_type);
        engine.init_cabac();
        engine
    }

    pub fn init_cabac(&mut self) {
        // Ensure we have at least 2 bytes available from the current cursor
        if self.data.len() >= self.cursor + 2 {
            // 1. Read 16 bits starting from the current cursor (byte-aligned)
            // This is the 'iv' value in the spec (Initial Value)
            self.value = (self.data[self.cursor] as u32) << 8 | (self.data[self.cursor + 1] as u32);

            // 2. Advance cursor by 2 bytes
            self.cursor += 2;

            // 3. Reset the Arithmetic range to 510 as per Section 9.3.2.2
            self.range = 510;

            // 4. bits_needed = -8 indicates that the next 'renorm'
            // will trigger a read of the next byte.
            self.bits_needed = -8;

            debug_more!(
                "init_CABAC_decode_2 range :{} value :{} cursor: {}",
                self.range,
                self.value,
                self.cursor
            );
        } else {
            // Handle end of stream / error case
            debug_more!("init_CABAC_decode_2: NOT ENOUGH DATA");
        }
    }

    /// Batch renorm: shifts value/range by `shift` bits and reads at most one
    /// new byte. Since shift <= 6 in the LPS path and bits_needed starts at
    /// -8, a single byte always covers the demand.
    #[inline(always)]
    fn renorm(&mut self, shift: u32) {
        self.value <<= shift;
        self.bits_needed += shift as i8;

        if self.bits_needed >= 0 {
            // Refill the register from the bitstream
            let byte = if self.cursor < self.data.len() {
                let b = self.data[self.cursor];
                self.cursor += 1;
                b as u32
            } else {
                0 // Padding for trailing bits
            };

            // Align the new byte based on how many bits were already consumed
            self.value |= byte << self.bits_needed;
            self.bits_needed -= 8;
        }
    }

    #[inline(always)]
    fn renorm_one(&mut self) {
        self.renorm(1);
    }

    fn _print_states(&self) {
        for i in 100..NUM_CABAC_CONTEXTS {
            let state = self.contexts[i as usize];
            let mps = state & 1;
            let state = state >> 1;
            println!("i={i},mps:{},state:{}", mps, state);
        }
    }
    // --- Core Decoding Functions ---

    #[inline(always)]
    pub fn decode_decision(&mut self, ctx_idx: usize) -> u8 {
        let state_packed = self.contexts[ctx_idx];
        let mps = state_packed & 1;
        let state = (state_packed >> 1) as usize;

        debug_more!(
            "decodeBin range :{} value:{} state:{},ctx_idx:{}",
            self.range,
            self.value,
            state,
            ctx_idx
        );

        let q_idx = (self.range >> 6) & 3;
        let lps_range = RANGE_LPS_TABLE[state][q_idx as usize] as u32;

        self.range -= lps_range;
        let scaled_range = self.range << 7;

        debug_more!(" sr:{} v:{}", scaled_range, self.value);

        if self.value < scaled_range {
            // --- MPS path ---
            debug_more!(" MPS");
            self.contexts[ctx_idx] = (TRANSITION_MPS[state] << 1) | mps;

            if self.range < 256 {
                self.range <<= 1;
                self.renorm_one();
            }

            debug_more!(" -> bit {}  r:{} v:{}", mps, self.range, self.value);
            mps
        } else {
            // --- LPS path ---
            // Assuming lps_decode is a helper or inline logic:
            self.lps_decode(ctx_idx, mps, state, lps_range, scaled_range)
        }
    }

    /// Internal LPS logic to keep decode_decision slim
    #[inline(never)]
    fn lps_decode(
        &mut self, ctx_idx: usize, mps: u8, state: usize, lps_range: u32, scaled_range: u32
    ) -> u8 {
        debug_more!(" LPS");
        let bin = 1 - mps;
        self.value -= scaled_range;

        let shift = RENORM_TABLE[(lps_range >> 3) as usize] as u32;
        self.range = lps_range << shift;
        self.renorm(shift);

        let next_mps = if state == 0 { 1 - mps } else { mps };
        self.contexts[ctx_idx] = (TRANSITION_LPS[state] << 1) | next_mps;

        debug_more!(" -> bit {}  r:{} v:{}", bin, self.range, self.value);
        bin
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

    /// Optimized: Decode `n_bits` bypass bins in one pass.
    pub fn decode_fl_bypass(&mut self, n_bits: u8) -> u32 {
        debug_more!(
            "bypass group r:{},v:{} (n_bits={})",
            self.range,
            self.value,
            n_bits
        );
        if n_bits == 0 {
            return 0;
        }

        // Renorm all bits at once
        self.renorm(n_bits as u32);

        let scaled = self.range << 7;
        let mut res = 0u32;

        // Peel bits off from MSB to LSB
        for i in (0..n_bits).rev() {
            if self.value >= (scaled << i) {
                // Account for the batched shift in value
                self.value -= scaled << i;
                res |= 1 << i;
            }
        }
        debug_more!("  FL: {}", res);
        res
    }

    pub fn decode_bypass_eg0(&mut self) -> u32 {
        let mut prefix = 0u32;
        while self.decode_bypass() == 1 {
            prefix += 1;
            if prefix > 32 {
                break;
            } // Safety break
        }

        if prefix == 0 {
            return 0;
        }

        // Optimized suffix read using the FL batch helper
        let suffix = self.decode_fl_bypass(prefix as u8);
        (1 << prefix) - 1 + suffix
    }

    pub fn decode_tu_bypass(&mut self, c_max: u8) -> u8 {
        for i in 0..c_max {
            if self.decode_bypass() == 0 {
                return i;
            }
        }
        c_max
    }
}

impl<'a> CabacDecoder<'a> {
    pub fn init_contexts(&mut self, qp: i32, init_type: usize) {
        let qp_y = qp.clamp(0, 51);

        // --- 1. MOTION CONTEXTS (P/B Slices Only) ---
        // libde265 initType: 0=I, 1=P, 2=B.
        // Based on your code, your mapping is: 0=B, 1=P, 2=I.
        // We must ensure we adjust the 'init_type' used as index for the C++ tables.
        if init_type > 0 {
            // Only for B (0) or P (1)
            let lib_idx = if init_type == 1 { 0 } else { 1 }; // P=0, B=1 for motion tables

            self.set_init(qp_y, CONTEXT_MODEL_CU_SKIP_FLAG, &INIT_CU_SKIP[lib_idx], 3);
            self.set_init(
                qp_y,
                CONTEXT_MODEL_PRED_MODE_FLAG,
                &[INIT_PRED_MODE[lib_idx]],
                1
            );
            self.set_init(
                qp_y,
                CONTEXT_MODEL_MERGE_FLAG,
                &[INIT_MERGE_FLAG[lib_idx]],
                1
            );
            self.set_init(qp_y, CONTEXT_MODEL_MERGE_IDX, &[INIT_MERGE_IDX[lib_idx]], 1);
            self.set_init(qp_y, CONTEXT_MODEL_INTER_PRED_IDC, &INIT_INTER_PRED_IDC, 5);
            self.set_init(qp_y, CONTEXT_MODEL_REF_IDX_LX, &INIT_REF_IDX, 2);

            let mvd_idx = if init_type == 1 { 0 } else { 2 }; // P=0, B=2
            self.set_init(
                qp_y,
                CONTEXT_MODEL_ABS_MVD_GREATER01_FLAG,
                &INIT_ABS_MVD[mvd_idx..],
                2
            );

            self.set_init(qp_y, CONTEXT_MODEL_MVP_LX_FLAG, &INIT_MVP_LX, 1);
            self.set_init(qp_y, CONTEXT_MODEL_RQT_ROOT_CBF, &INIT_RQT_ROOT, 1);

            self.set_init_const(qp_y, CONTEXT_MODEL_RDPCM_FLAG, 139, 2);
            self.set_init_const(qp_y, CONTEXT_MODEL_RDPCM_DIR, 139, 2);
        }

        // --- 2. COMMON CONTEXTS (All Slices) ---
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

        // --- 3. RESIDUALS / COEFFICIENTS ---
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

        // Significance flags (42 + 2)
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

        // !!! START OF MISSING DATA (i=109+) !!!

        // Coefficient Absolute Levels (Greater than 1 and 2)
        self.set_init(
            qp_y,
            CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG,
            &INIT_GTR_1[init_type * 24..],
            24
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG,
            &INIT_GTR_2[init_type * 6..],
            6
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

        // Quantization and Transform
        self.set_init(qp_y, CONTEXT_MODEL_CU_QP_DELTA_ABS, &INIT_QP_DELTA, 2);
        self.set_init(
            qp_y,
            CONTEXT_MODEL_TRANSFORM_SKIP_FLAG,
            &INIT_TRANSFORM_SKIP,
            2
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG,
            &[INIT_TRANSQUANT_BYPASS[init_type]],
            1
        );

        // Constant Initializations (Standard HEVC values, usually 154)
        self.set_init_const(qp_y, CONTEXT_MODEL_LOG2_RES_SCALE_ABS_PLUS1, 154, 8);
        self.set_init_const(qp_y, CONTEXT_MODEL_RES_SCALE_SIGN_FLAG, 154, 2);
        self.set_init_const(qp_y, CONTEXT_MODEL_CU_CHROMA_QP_OFFSET_FLAG, 154, 1);
        self.set_init_const(qp_y, CONTEXT_MODEL_CU_CHROMA_QP_OFFSET_IDX, 154, 1);
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

    // Updated helper to handle multiple constants
    fn set_init_const(&mut self, qp: i32, idx: usize, iv: u8, len: usize) {
        let values = vec![iv; len];
        self.set_init(qp, idx, &values, len);
    }
}
