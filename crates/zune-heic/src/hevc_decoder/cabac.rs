use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_ABS_MVD_GREATER01_FLAG, CONTEXT_MODEL_CBF_CHROMA, CONTEXT_MODEL_CBF_LUMA,
    CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG, CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG,
    CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG, CONTEXT_MODEL_CU_CHROMA_QP_OFFSET_FLAG,
    CONTEXT_MODEL_CU_CHROMA_QP_OFFSET_IDX, CONTEXT_MODEL_CU_QP_DELTA_ABS,
    CONTEXT_MODEL_CU_SKIP_FLAG, CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG,
    CONTEXT_MODEL_INTER_PRED_IDC, CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE,
    CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX,
    CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX, CONTEXT_MODEL_LOG2_RES_SCALE_ABS_PLUS1,
    CONTEXT_MODEL_MERGE_FLAG, CONTEXT_MODEL_MERGE_IDX, CONTEXT_MODEL_MVP_LX_FLAG,
    CONTEXT_MODEL_PART_MODE, CONTEXT_MODEL_PRED_MODE_FLAG, CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG,
    CONTEXT_MODEL_RDPCM_DIR, CONTEXT_MODEL_RDPCM_FLAG, CONTEXT_MODEL_REF_IDX_LX,
    CONTEXT_MODEL_RES_SCALE_SIGN_FLAG, CONTEXT_MODEL_RQT_ROOT_CBF, CONTEXT_MODEL_SAO_MERGE_FLAG,
    CONTEXT_MODEL_SAO_TYPE_IDX, CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG, CONTEXT_MODEL_SPLIT_CU_FLAG,
    CONTEXT_MODEL_SPLIT_TRANSFORM_FLAG, CONTEXT_MODEL_TRANSFORM_SKIP_FLAG, INIT_ABS_MVD,
    INIT_CBF_CHROMA, INIT_CBF_LUMA, INIT_CHROMA_PRED, INIT_CODED_SUB, INIT_CU_SKIP, INIT_GTR_1,
    INIT_GTR_2, INIT_INTER_PRED_IDC, INIT_LAST_COEFF, INIT_MERGE_FLAG, INIT_MERGE_IDX, INIT_MVP_LX,
    INIT_PART_MODE, INIT_PRED_MODE, INIT_PREV_INTRA, INIT_QP_DELTA, INIT_REF_IDX, INIT_RQT_ROOT,
    INIT_SAO_MERGE, INIT_SAO_TYPE, INIT_SIG_COEFF, INIT_SIG_COEFF_SKIP, INIT_SPLIT_CU,
    INIT_SPLIT_TRANS, INIT_TRANSFORM_SKIP, INIT_TRANSQUANT_BYPASS, RANGE_LPS_TABLE_1D,
    RENORM_TABLE, TRANSITION_LPS, TRANSITION_MPS,
};

pub const NUM_CABAC_CONTEXTS: usize = 172;

// --- ENGINE IMPLEMENTATION ---

pub struct CabacDecoder<'a> {
    pub data: &'a [u8],
    pub cursor: usize,
    pub range: u32,
    pub value: u32,
    pub bits_needed: i32,
    // ---  64-bit Bit Reservoir ---
    pub cache: u64,
    pub cache_bytes: u32,
    pub contexts: [u8; NUM_CABAC_CONTEXTS],
}

impl<'a> CabacDecoder<'a> {
    pub fn new(data: &'a [u8], slice_qp: i32, init_type: usize) -> Self {
        let mut engine = Self {
            data,
            cursor: 0,
            range: 510,
            value: 0,
            bits_needed: -8,
            cache: 0,
            cache_bytes: 0,
            contexts: [0; NUM_CABAC_CONTEXTS],
        };

        engine.init_contexts(slice_qp, init_type);
        engine.init_cabac();
        engine
    }
    #[inline(never)]
    fn fill_cache(&mut self) {
        debug_assert!(self.cache_bytes == 0);
        match self.data.get(self.cursor..self.cursor + 8) {
            None => {
                let remaining = (self.data.len() - self.cursor).min(7);

                let mut tmp = [0u8; 8];

                tmp[..remaining].copy_from_slice(&self.data[self.cursor..]);

                self.cache = u64::from_be_bytes(tmp);
                self.cache_bytes = remaining as u32;
                self.cursor = self.data.len();
            }
            Some(bytes) => {
                let chunk = bytes.try_into().unwrap();

                self.cache = u64::from_be_bytes(chunk);
                self.cache_bytes = 8;
                self.cursor += 8;
            }
        }
    }
    /// Extracts 1 byte entirely from the CPU register cache.
    #[inline(always)]
    fn read_byte(&mut self) -> u32 {
        if self.cache_bytes == 0 {
            self.fill_cache();
            if self.cache_bytes == 0 {
                return 0; // Padding for trailing bits
            }
        }
        // Extract the highest byte from the u64
        let b = (self.cache >> 56) as u32;
        // Slide the window up by 8 bits
        self.cache <<= 8;
        self.cache_bytes -= 1;

        b
    }

    pub fn init_cabac(&mut self) {
        // Ensure we have at least 2 bytes available from the current cursor
        if self.data.len() >= self.cursor + 2 {
            // 1. Read 16 bits starting from the current cursor (byte-aligned)
            // This is the 'iv' value in the spec (Initial Value)
            self.value = (self.read_byte() << 8) | self.read_byte();

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
        self.value = self.value.wrapping_shl(shift);
        self.bits_needed += shift as i32;

        // Refill the register from the bitstream
        if self.bits_needed >= 0 {
            // Refill the register from the bitstream
            let byte = self.read_byte();

            // Align the new byte based on how many bits were already consumed
            self.value |= byte << self.bits_needed;
            self.bits_needed -= 8;
        }
    }

    #[inline(always)]
    fn renorm_one(&mut self) {
        self.renorm(1);
    }

    // --- Core Decoding Functions ---

    #[inline(always)]
    pub fn decode_decision(&mut self, ctx_idx: usize) -> u8 {
        // NB: CAE this is a very hot code path, so some optimizations have been
        // applied
        debug_assert!(ctx_idx < NUM_CABAC_CONTEXTS, "Invalid ctx_idx");

        // small optimization to fetch context once saving us a branch check below when it
        // was in mps mode (branch check was for panic as the code does something like
        // self.contexts[ctx_idx]=value.
        // dummy is here to elide the value dropped before code
        let mut dummy = 0;
        let ctx_v = self.contexts.get_mut(ctx_idx).unwrap_or(&mut dummy);
        let state_packed = *ctx_v;

        let mps = state_packed & 1;
        let bin = 1 - mps;
        let state = (state_packed >> 1) as usize;

        // another optimization
        // Move the bit logic on top of here to better pipeline
        // I am not sure if the compiler again is doing this
        let next_mps = mps ^ u8::from(state == 0);

        let mps_bit = (TRANSITION_MPS[state & 63] << 1) | mps;
        let lps_bit = (TRANSITION_LPS[state & 63] << 1) | next_mps;

        debug_more!(
            "decodeBin range :{} value:{} state:{},ctx_idx:{}",
            self.range,
            self.value,
            state,
            ctx_idx
        );

        let q_idx = (self.range >> 6) & 3;
        let flat_idx = (state << 2) | (q_idx as usize);

        let lps_range = u32::from(RANGE_LPS_TABLE_1D[flat_idx & 0xFF]);

        self.range -= lps_range;
        let scaled_range = self.range << 7;

        debug_more!(" sr:{} v:{}", scaled_range, self.value);

        let mps_side = self.value < scaled_range;

        if mps_side {
            *ctx_v = mps_bit;

            debug_more!("MPS");
            // check range
            if self.range < 256 {
                self.range <<= 1;
                self.renorm_one();
            }
            debug_more!(" -> bit {}  r:{} v:{}", mps, self.range, self.value);

            mps
        } else {
            *ctx_v = lps_bit;

            debug_more!("LPS");
            // lps side
            self.value -= scaled_range;

            let range = (lps_range >> 3) as usize;
            let shift = u32::from(RENORM_TABLE[range & 31]);
            // just check that the & 31 optimization was valid but for debug builds
            debug_assert!(
                range < RENORM_TABLE.len(),
                "range in LPS would panic {} {}",
                range,
                RENORM_TABLE.len()
            );

            self.range = lps_range << shift;
            self.renorm(shift);
            debug_more!(" -> bit {}  r:{} v:{}", bin, self.range, self.value);

            bin
        }
    }

    #[inline(always)]
    pub fn decode_bypass(&mut self) -> u8 {
        debug_more!("bypass r:{} v:{}", self.range, self.value);
        self.renorm_one();

        let scaled_range = self.range << 7;
        let bit = u8::from(self.value >= scaled_range);
        // CAE: (branchless, not sure if the optimizer was doing
        // it :)
        self.value -= scaled_range * u32::from(bit);

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
    /// Optimized: Decode `n_bits` bypass bins.
    #[inline]
    pub fn decode_fl_bypass(&mut self, mut n_bits: u8) -> u32 {
        if n_bits == 0 {
            return 0;
        }

        // Fast path: renorm() safely handles up to 8 bits in one go.
        if n_bits <= 8 {
            return self.decode_fl_bypass_batched(n_bits);
        }

        // Fallback for n_bits > 8: Batch the first 8, loop the rest.
        let mut res = self.decode_fl_bypass_batched(8);
        n_bits -= 8;

        while n_bits > 0 {
            res <<= 1;
            res |= u32::from(self.decode_bypass());
            n_bits -= 1;
        }

        res
    }

    /// Internal helper that strictly expects n_bits <= 8
    #[inline(always)]
    fn decode_fl_bypass_batched(&mut self, n_bits: u8) -> u32 {
        debug_assert!(n_bits <= 8, "Batched bypass exceeds 8 bits!");

        // Renorm all bits at once
        self.renorm(u32::from(n_bits));

        let scaled = self.range << 7;
        let mut res = 0u32;

        // Peel bits off from MSB to LSB
        for i in (0..n_bits).rev() {
            if self.value >= (scaled << i) {
                self.value -= scaled << i;
                res |= 1 << i;
            }
        }
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

impl CabacDecoder<'_> {
    #[allow(clippy::too_many_lines)]
    pub fn init_contexts(&mut self, qp: i32, init_type: usize) {
        let qp_y = qp.clamp(0, 51);

        // --- 1. MOTION CONTEXTS (P/B Slices Only) ---
        // libde265 initType: 0=I, 1=P, 2=B.
        if init_type > 0 {
            // Only for B (0) or P (1)
            let lib_idx = usize::from(init_type != 1); // P=0, B=1 for motion tables

            self.set_init(qp_y, CONTEXT_MODEL_CU_SKIP_FLAG, &INIT_CU_SKIP[lib_idx], 3);
            self.set_init(
                qp_y,
                CONTEXT_MODEL_PRED_MODE_FLAG,
                &[INIT_PRED_MODE[lib_idx]],
                1,
            );
            self.set_init(
                qp_y,
                CONTEXT_MODEL_MERGE_FLAG,
                &[INIT_MERGE_FLAG[lib_idx]],
                1,
            );
            self.set_init(qp_y, CONTEXT_MODEL_MERGE_IDX, &[INIT_MERGE_IDX[lib_idx]], 1);
            self.set_init(qp_y, CONTEXT_MODEL_INTER_PRED_IDC, &INIT_INTER_PRED_IDC, 5);
            self.set_init(qp_y, CONTEXT_MODEL_REF_IDX_LX, &INIT_REF_IDX, 2);

            let mvd_idx = if init_type == 1 { 0 } else { 2 }; // P=0, B=2
            self.set_init(
                qp_y,
                CONTEXT_MODEL_ABS_MVD_GREATER01_FLAG,
                &INIT_ABS_MVD[mvd_idx..],
                2,
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
            3,
        );

        let part_idx = if init_type == 2 { 5 } else { init_type };
        self.set_init(
            qp_y,
            CONTEXT_MODEL_PART_MODE,
            &INIT_PART_MODE[part_idx..],
            4,
        );

        self.set_init(
            qp_y,
            CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG,
            &[INIT_PREV_INTRA[init_type]],
            1,
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE,
            &[INIT_CHROMA_PRED[init_type]],
            1,
        );

        let cbf_l_idx = if init_type == 0 { 0 } else { 2 };
        self.set_init(qp_y, CONTEXT_MODEL_CBF_LUMA, &INIT_CBF_LUMA[cbf_l_idx..], 2);
        self.set_init(
            qp_y,
            CONTEXT_MODEL_CBF_CHROMA,
            &INIT_CBF_CHROMA[init_type * 4..],
            4,
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SPLIT_TRANSFORM_FLAG,
            &INIT_SPLIT_TRANS[init_type * 3..],
            3,
        );

        // --- 3. RESIDUALS / COEFFICIENTS ---
        self.set_init(
            qp_y,
            CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX,
            &INIT_LAST_COEFF[init_type * 18..],
            18,
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX,
            &INIT_LAST_COEFF[init_type * 18..],
            18,
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG,
            &INIT_CODED_SUB[init_type * 4..],
            4,
        );

        // Significance flags (42 + 2)
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG,
            &INIT_SIG_COEFF[init_type],
            42,
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG + 42,
            &INIT_SIG_COEFF_SKIP[init_type],
            2,
        );

        // !!! START OF MISSING DATA (i=109+) !!!

        // Coefficient Absolute Levels (Greater than 1 and 2)
        self.set_init(
            qp_y,
            CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG,
            &INIT_GTR_1[init_type * 24..],
            24,
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG,
            &INIT_GTR_2[init_type * 6..],
            6,
        );

        // SAO
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SAO_MERGE_FLAG,
            &[INIT_SAO_MERGE[init_type]],
            1,
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_SAO_TYPE_IDX,
            &[INIT_SAO_TYPE[init_type]],
            1,
        );

        // Quantization and Transform
        self.set_init(qp_y, CONTEXT_MODEL_CU_QP_DELTA_ABS, &INIT_QP_DELTA, 2);
        self.set_init(
            qp_y,
            CONTEXT_MODEL_TRANSFORM_SKIP_FLAG,
            &INIT_TRANSFORM_SKIP,
            2,
        );
        self.set_init(
            qp_y,
            CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG,
            &[INIT_TRANSQUANT_BYPASS[init_type]],
            1,
        );

        // Constant Initializations (Standard HEVC values, usually 154)
        self.set_init_const(qp_y, CONTEXT_MODEL_LOG2_RES_SCALE_ABS_PLUS1, 154, 8);
        self.set_init_const(qp_y, CONTEXT_MODEL_RES_SCALE_SIGN_FLAG, 154, 2);
        self.set_init_const(qp_y, CONTEXT_MODEL_CU_CHROMA_QP_OFFSET_FLAG, 154, 1);
        self.set_init_const(qp_y, CONTEXT_MODEL_CU_CHROMA_QP_OFFSET_IDX, 154, 1);
    }
    fn set_init(&mut self, qp: i32, start_idx: usize, values: &[u8], len: usize) {
        let qp_clipped = qp.clamp(0, 51);

        for i in 0..len {
            let iv = i32::from(values[i]);
            let slope_idx = iv >> 4;
            let intersec_idx = iv & 0xF;

            let m = slope_idx * 5 - 45;
            let n = (intersec_idx << 3) - 16;

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
