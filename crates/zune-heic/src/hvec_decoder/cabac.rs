use crate::debug_more;
use crate::hvec_decoder::DEBUG_MORE;
use crate::hvec_decoder::cabac_tables::*;

pub const NUM_CABAC_CONTEXTS: usize = 171;

// --- ENGINE IMPLEMENTATION ---

pub struct CabacEngine<'a> {
    data:          &'a [u8],
    cursor:        usize,
    buffered_bits: u64,
    bits_left:     u8,
    pub value:     u32,
    pub range:     u32,
    pub contexts:  [u8; NUM_CABAC_CONTEXTS] // Room for all HEVC contexts
}

impl<'a> CabacEngine<'a> {
    pub fn new(
        data: &'a [u8], slice_qp: i32, init_type: usize
    ) -> Self {
        let mut engine = Self {
            data,
            cursor: 0,
            range: 510,
            value: 0,
            buffered_bits: 0,
            bits_left: 0,
            contexts: [0; NUM_CABAC_CONTEXTS]
        };

        engine.init_contexts_libde265(slice_qp, init_type);
        engine.init_value();
        engine
    }

    fn init_value(&mut self) {
        if self.data.len() >= 2 {
            // Read first two bytes: value = byte0 << 8 | byte1
            self.value = (self.data[0] as u32) << 8 | (self.data[1] as u32);
            self.cursor = 2;
            self.bits_left = 0; // Forces refill_bulk on the next bit read
            self.buffered_bits = 0;
        }
    }

    #[inline(always)]
    fn read_n_bits(&mut self, n: u8) -> u32 {
        if n == 0 {
            return 0;
        }
        if self.bits_left < n {
            self.refill_bulk();
        }

        let bits = (self.buffered_bits >> (64 - n)) as u32;
        self.buffered_bits <<= n;
        self.bits_left -= n;
        bits
    }

    #[inline(never)]
    fn refill_bulk(&mut self) {
        let bytes_available = self.data.len().saturating_sub(self.cursor);
        let bytes_to_read = bytes_available.min(8);

        if bytes_to_read > 0 {
            let mut word = 0u64;
            for i in 0..bytes_to_read {
                word |= (self.data[self.cursor + i] as u64) << (56 - i * 8);
            }

            // Shift into the empty part of the buffer
            self.buffered_bits |= word >> self.bits_left;
            self.cursor += bytes_to_read;
            self.bits_left += (bytes_to_read * 8) as u8;
        }
    }

    pub fn _print_states(&self) {
        for (i, x) in self.contexts.iter().enumerate() {
            let mps = x & 1;
            let state = (x >> 1) as usize;
            debug_more!("{i} mps: {} state: {}", mps, state);
        }
    }

    pub fn decode_decision(&mut self, ctx_idx: usize) -> u8 {
        let state_packed = self.contexts[ctx_idx];
        let mps = state_packed & 1;
        let state = (state_packed >> 1) as usize;

        debug_more!(
            "decode_bin range:{} value:{} state:{}",
            self.range,
            self.value,
            state
        );
        let q_idx = (self.range >> 6) & 3;
        let lps_range = RANGE_LPS_TABLE[state][q_idx as usize] as u32;

        self.range -= lps_range;
        let scaled_range = self.range << 7;

        debug_more!(
            " decode_bin[1] scaled_range:{} value:{} ",
            scaled_range,
            self.value
        );

        let bin;
        if self.value < scaled_range {
            // MPS Path
            bin = mps;
            debug_more!(" decode_bin[2] MPS");
            self.contexts[ctx_idx] = (TRANSITION_MPS[state] << 1) | mps;

            if self.range < 256 {
                self.range <<= 1;
                self.value = self.value << 1;

                let new_bits = self.read_n_bits(1);

                self.value |= new_bits;
            }
        } else {
            // LPS Path
            bin = 1 - mps;
            self.value -= scaled_range;

            let shift = RENORM_TABLE[(lps_range >> 3) as usize] as u32;
            self.value <<= shift;
            self.value |= self.read_n_bits(shift as u8);
            self.range = lps_range << shift;

            let next_mps = if state == 0 { 1 - mps } else { mps };
            self.contexts[ctx_idx] = (TRANSITION_LPS[state] << 1) | next_mps;
        }
        debug_more!(
            " decode_bin[3] MPS bit {} range:{} value:{}",
            bin,
            self.range,
            self.value
        );
        bin
    }
    pub fn decode_fl_bypass_parallel(&mut self, n_bits: u8) -> u32 {
        debug_more!(
            "decode_bypass_parallel range={} value={} (n_bits={})",
            self.range,
            self.value,
            n_bits
        );
        self.value <<= n_bits;
        self.bits_left -= n_bits;

        let scaled_range = self.range << 7;
        let v = self.value / scaled_range;
        self.value -= v * scaled_range;

        debug_more!(
            " decode_bypass_parallel d={} range={} value={} ",
            v,
            scaled_range,
            self.value
        );
        v
    }
    pub fn decode_fl_bypass(&mut self, mut n_bits: u8) -> u32 {
        let mut v;

        if n_bits == 0 {
            return 0;
        }
        if n_bits == 1 {
            v = self.decode_bypass() as u32;
        } else {
            v = self.decode_fl_bypass_parallel(8);
            n_bits -= 8;

            while n_bits > 0 {
                v <<= 1;
                v |= self.decode_bypass() as u32;
                n_bits -= 1;
            }
        }
        debug_more!("decode_fl_bypass v={}", v);
        return v;
    }

    pub fn decode_bypass(&mut self) -> u8 {
        debug_more!("decode_bypass range:{} value:{}", self.range, self.value);
        self.value = (self.value << 1) | self.read_n_bits(1);
        let scaled_range = self.range << 7;

        let return_value = if self.value >= scaled_range {
            self.value -= scaled_range;
            1
        } else {
            0
        };
        debug_more!(
            " decode_bypass[2] bit:{} range:{},value:{}",
            return_value,
            self.range,
            self.value
        );
        return_value
    }

    pub fn decode_terminate(&mut self) -> u8 {
        self.range -= 2;
        let scaled_range = self.range << 7;

        if self.value >= scaled_range {
            1
        } else {
            if self.range < 256 {
                self.range <<= 1;
                self.value = (self.value << 1) | self.read_n_bits(1);
            }
            0
        }
    }
    pub fn decode_tu_bypass(&mut self, c_max: u8) -> u8 {
        for i in 0..c_max {
            let bit = self.decode_bypass();
            if bit == 0 {
                return i;
            }
        }
        return c_max;
    }
}

impl<'a> CabacEngine<'a> {
    pub fn init_contexts_libde265(&mut self, qp: i32, init_type: usize) {
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
