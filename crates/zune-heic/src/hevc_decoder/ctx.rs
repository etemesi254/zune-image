use std::sync::Arc;

use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac::{CabacDecoder, NUM_CABAC_CONTEXTS};
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, Pps, SliceHeader, Sps};
use crate::hevc_decoder::neighbor_tracker::NeighborTracker;
use crate::hevc_decoder::quadtree::sao::SaoInfo;
use crate::hevc_decoder::quadtree::sig_ctx_generator::generate_all_sig_ctx_maps;
use crate::hevc_decoder::raw_frame::{RawFrame, SingleFrame};

pub struct DecodeSliceContext<'a> {
    pub sps:                    &'a Sps,
    pub pps:                    &'a Pps,
    pub slice_header:           &'a SliceHeader,
    pub cabac:                  CabacDecoder<'a>,
    pub neighbor_tracker:       &'a mut NeighborTracker,
    pub is_cu_qp_delta_coded:   bool,
    pub cu_qp_delta:            i32,
    // quantization group
    pub current_qg_x:           usize,
    pub current_qg_y:           usize,
    pub last_qp_in_slice:       i8, // This tracks the "previous" QP for the next CU
    pub last_qp_in_previous_qg: i8,

    // - CU state to be captured for the tracker
    pub is_skip:                   bool,
    pub is_intra:                  bool,
    pub intra_mode_luma:           u8,
    pub intra_mode_chroma:         u8,
    pub qp_y_prime:                i32,
    pub qp_cb_prime:               i32,
    pub qp_cr_prime:               i32,
    // residual data
    pub cu_transquant_bypass_flag: bool,
    pub explicit_rdpcm_flag:       bool,
    pub explicit_rdpcm_dir:        u8,
    pub transform_skip_flag:       [u8; 3],
    // context significant maps
    pub sig_ctx_maps:              Vec<Vec<Vec<Vec<Vec<u8>>>>>,
    pub stat_coeff:                [u8; 4],
    pub coeff_list:                [[i16; 32 * 32]; 3],
    pub coeff_pos:                 [[i16; 32 * 32]; 3],
    pub n_coeff:                   [i16; 3],
    // raw image frame reference
    pub raw_frame:                 Arc<RawFrame>,
    // --- scratch buffers
    // --- High-Speed Fixed Buffers ---
    pub pixel_scratchpad:          Vec<u8>,
    // Use i32 so it's large enough for Scaling, IDCT, and RDPCM
    pub math_scratchpad:           Vec<i16>,
    // idct scratchpad instead of allocating
    pub idct_scratchpad:           Vec<i16>,
    // Reference Wall buffers
    pub ref_samples_p:             Vec<u8>,
    pub ref_samples_available:     Vec<bool>,
    pub ref_main_buf:              Vec<u8>,
    pub res_scale_val:             i8,
    ///  Stores the Luma residuals for the current TU area
    /// so Chroma can use them for CCP.
    pub luma_residual_temp:        Vec<i16>,

    pub ctb_sao_buffer: Vec<SaoInfo>,
    // ctb contexts
    pub ctb_context:    Vec<Option<[u8; NUM_CABAC_CONTEXTS]>>
}
impl<'a> DecodeSliceContext<'a> {
    pub fn new(
        sps: &'a Sps, pps: &'a Pps, slice_header: &'a SliceHeader, cabac_engine: CabacDecoder<'a>,
        neighbor_tracker: &'a mut NeighborTracker, last_qp_in_slice: i8, raw_frame: Arc<RawFrame>
    ) -> Self {
        // SAO data

        let ctb_size = 1 << sps.log2_ctb_size_y;

        let width_in_ctbs = sps.pic_width_in_luma_samples.div_ceil(ctb_size);
        let height_in_ctbs = sps.pic_height_in_luma_samples.div_ceil(ctb_size);

        let buffer_size = (width_in_ctbs * height_in_ctbs) as usize;

        Self {
            sps,
            pps,
            slice_header,
            cabac: cabac_engine,
            neighbor_tracker,
            last_qp_in_slice,
            is_cu_qp_delta_coded: false,
            cu_qp_delta: 0,
            current_qg_x: usize::MAX,
            current_qg_y: usize::MAX,
            last_qp_in_previous_qg: 0,
            is_skip: false,
            is_intra: false,
            intra_mode_luma: 0,
            intra_mode_chroma: 0,
            cu_transquant_bypass_flag: false,
            qp_y_prime: 0,
            qp_cb_prime: 0,
            qp_cr_prime: 0,
            transform_skip_flag: [0; 3],
            explicit_rdpcm_flag: false,
            explicit_rdpcm_dir: 0,
            sig_ctx_maps: generate_all_sig_ctx_maps(),
            stat_coeff: [0; 4],
            coeff_list: [[0; 32 * 32]; 3],
            coeff_pos: [[0; 32 * 32]; 3],
            n_coeff: [0; 3],
            raw_frame,
            pixel_scratchpad: vec![0; 1024],
            ref_main_buf: vec![0; 97],
            ref_samples_p: vec![0; 129],
            ref_samples_available: vec![false; 129],
            math_scratchpad: vec![0; 1024],
            luma_residual_temp: vec![0; 1024],
            idct_scratchpad: vec![0; 1024],
            res_scale_val: -1,
            ctb_sao_buffer: vec![SaoInfo::default(); buffer_size],
            ctb_context: vec![None; height_in_ctbs as usize]
        }
    }
}

impl DecodeSliceContext<'_> {
    pub fn set_sao_info(&mut self, x_ctb: usize, y_ctb: usize, info: SaoInfo) {
        let width = self.sps.pic_width_in_ctbs_y as usize;
        let addr = y_ctb * width + x_ctb;
        // Assuming ctb_info is a Vec<SaoInfo> in your context
        self.ctb_sao_buffer[addr] = info;
    }

    pub fn get_neighbor_sao(&self, x_ctb: usize, y_ctb: usize) -> &SaoInfo {
        let width = self.sps.pic_width_in_ctbs_y as usize;
        let addr = y_ctb * width + x_ctb;
        &self.ctb_sao_buffer[addr]
    }
}
impl DecodeSliceContext<'_> {
    /// Sets a block of pixels (e.g., after reconstruction)
    ///
    /// Data is expected to be in scratchpad
    pub fn write_block_scratchpad(
        &self, c_idx: usize, x0: usize, y0: usize, n_t: usize, bit_depth: u8
    ) {
        let mut plane = match c_idx {
            0 => self.raw_frame.luma.lock().unwrap(),
            1 => self.raw_frame.cb.lock().unwrap(),
            2 => self.raw_frame.cr.lock().unwrap(),
            _ => panic!("Invalid component index")
        };

        write_block_and_pad(
            &mut plane,
            x0,
            y0,
            n_t,
            &self.pixel_scratchpad,
            None,
            bit_depth
        );
    }

    #[allow(clippy::too_many_lines)]
    pub fn scale_coefficients(
        &mut self,
        x_t: usize,
        y_t: usize, // TU pos
        n_t: usize, // TU size (4, 8, 16, 32)
        c_idx: usize
    ) {
        const LEVEL_SCALE: [i32; 6] = [40, 45, 51, 57, 64, 72];

        debug_more!(
            "-----------scale_coefficients :xT={} yT={} n_t={} cidx={}-----------",
            x_t,
            y_t,
            n_t,
            c_idx
        );

        let sps = self.sps;
        let pps = self.pps;

        // 1. Get the QP for this component
        let qp = match c_idx {
            0 => self.qp_y_prime.abs(),
            1 => self.qp_cb_prime.abs(),
            2 => self.qp_cr_prime.abs(),
            _ => unreachable!()
        };
        debug_more!("qp:{}", qp);

        // 2. Clear the coefficient buffer for this TU
        self.math_scratchpad[..n_t * n_t].fill(0);

        // 3. Lossless Check (Transquant Bypass)
        if self.cu_transquant_bypass_flag {
            for i in 0..self.n_coeff[c_idx] {
                let pos = self.coeff_pos[c_idx][i as usize] as usize;
                let level = self.coeff_list[c_idx][i as usize];
                self.math_scratchpad[pos] = level;
            }
            return;
        }

        // 4. Calculate bdShift (Spec 8.6.3)
        let bit_depth = if c_idx == 0 { sps.bit_depth_luma } else { sps.bit_depth_chroma };
        let log2_n_t = n_t.trailing_zeros() as i32;
        let mut bd_shift = i32::from(bit_depth) + log2_n_t - 5;

        // 5. Scaling Logic
        if sps.scaling_list_enabled_flag {
            // --- Custom Scaling Lists ---
            debug_more!("bd_shift:{}", bd_shift);
            let offset = 1 << (bd_shift - 1);
            // code for getting scaling list.
            // its duplicated here because rust would complain that it is borrowed twice
            let mut matrix_id = c_idx;

            let scaling_list = {
                let pps = self.pps;

                if n_t == 32 {
                    // 32x32 only has Luma IDs (0 for Intra, 1 for Inter)
                    matrix_id = 0;
                    if !self.is_intra {
                        matrix_id = 1;
                    }
                } else {
                    // 4x4, 8x8, 16x16 have 3 Intra followed by 3 Inter matrices
                    if !self.is_intra {
                        matrix_id += 3;
                    }
                }

                // 2. Return the correct buffer based on size
                match n_t {
                    4 => pps.pic_scaling_lists.size0[matrix_id].as_ref(), // [6][16]
                    8 => pps.pic_scaling_lists.size1[matrix_id].as_ref(), // [6][64]
                    16 => pps.pic_scaling_lists.size2[matrix_id].as_ref(), // [6][256]
                    32 => pps.pic_scaling_lists.size3[matrix_id].as_ref(), // [2][1024]
                    _ => unreachable!("Invalid TU size for scaling list")
                }
            };

            for i in 0..self.n_coeff[c_idx] {
                let pos = self.coeff_pos[c_idx][i as usize] as usize;
                let level = i64::from(self.coeff_list[c_idx][i as usize]);

                // --- FIX: Map NxN position to 8x8 scaling list ---
                let mut m_x_y = if n_t <= 8 {
                    i64::from(scaling_list[pos])
                } else {
                    let x = pos % n_t;
                    let y = pos / n_t;
                    let ratio = n_t >> 3; // 2 for 16x16, 4 for 32x32
                    i64::from(scaling_list[(y / ratio) * 8 + (x / ratio)])
                };

                // Special Case: DC Coefficient (Spec 8.6.3)
                if pos == 0 {
                    if n_t == 16 {
                        m_x_y = i64::from(pps.pic_scaling_lists.dc16[matrix_id]);
                    } else if n_t == 32 {
                        m_x_y = i64::from(pps.pic_scaling_lists.dc32[matrix_id]);
                    }
                }

                let fact = (m_x_y * i64::from(LEVEL_SCALE[(qp % 6) as usize])) << (qp / 6);
                let scaled = (level * fact + i64::from(offset)) >> bd_shift;

                let final_clipped = scaled.clamp(-32768, 32767);
                if DEBUG_MORE {
                    println!(
                        "TRACE_SCALE: i={i:>2} pos={pos:>4} level={level:>4} m_x_y={m_x_y:>3} fact={fact:>8} bdShift={bd_shift:>2} final={final_clipped:>5}"
                    );
                }

                self.math_scratchpad[pos] = final_clipped as i16;
            }
        } else {
            // Default Scaling (m_x_y = 16)
            bd_shift -= 4;
            debug_more!("bd_shift:{}", bd_shift);
            let offset = 1 << (bd_shift - 1);
            let fact = LEVEL_SCALE[(qp % 6) as usize] << (qp / 6);

            for i in 0..self.n_coeff[c_idx] {
                let pos = self.coeff_pos[c_idx][i as usize] as usize;
                let level = i64::from(self.coeff_list[c_idx][i as usize]);

                // The actual scaling math
                let scaled = (level * i64::from(fact) + i64::from(offset)) >> bd_shift;

                if DEBUG_MORE {
                    println!(
                        "TRACE_SCALE: i={:>2} pos={:>4} level={:>4}  fact={:>8} bdShift={:>2} final={:>5}",
                        i,
                        pos,
                        level,
                        fact,
                        bd_shift,
                        scaled.clamp(-32768, 32767),
                    );
                }
                // Clip to 16-bit range
                self.math_scratchpad[pos] = scaled.clamp(-32768, 32767) as _;
            }
        }
        // --- do transform or skip ---

        // Note: We only print if n_t is 4 or 8 to prevent overwhelming the console.
        // In a real debug session, you might remove this check.
        if DEBUG_MORE && n_t <= 32 {
            println!("coefficients OUT (cIdx:{c_idx} at {x_t},{y_t} size:{n_t}):");
            for y in 0..n_t {
                print!("  ");
                for x in 0..n_t {
                    // In your Rust port, coeffStride is just n_t since coeff_buffer is 1D
                    let val = self.math_scratchpad[y * n_t + x];
                    print!("{val:3} ");
                }
                println!();
            }
        }
    }
    /// Returns true if the Chroma Intra Prediction mode is DM_CHROMA (Mode 4).
    /// This is used to gate Cross-Component Prediction (CCP).
    pub fn is_intra_pred_mode_c_mode4(&self, x0: usize, y0: usize) -> bool {
        // In the HEVC spec, intra_chroma_pred_mode == 4 signifies Derived Mode.
        // This is usually stored in the neighbor tracker or a specific intra mode buffer.
        self.neighbor_tracker.get_intra_mode_chroma(x0, y0) == 4
    }

    fn rotate_coefficients_4x4(&mut self) {
        // Spec 7.4.9.11: Rotate by 180 degrees
        self.math_scratchpad[..16].reverse();
    }
    pub fn reconstruct_lossless(
        &mut self, x_t: usize, y_t: usize, n_t: usize, c_idx: usize, rdpcm_mode: u8
    ) {
        // --- 1. Extract values to the stack to avoid self-borrow conflicts ---
        let res_scale = self.res_scale_val;
        let n_coeffs = self.n_coeff[c_idx] as usize;

        // Range extension check
        let transform_skip_rotation_enabled = self
            .sps
            .range_extension
            .as_ref()
            .is_some_and(|rext| rext.transform_skip_rotation_enabled_flag);

        let rotate_coeffs = transform_skip_rotation_enabled && n_t == 4 && self.is_intra;

        // --- 2. Scatter coefficients ---
        // Access math_scratchpad directly
        self.math_scratchpad[..n_t * n_t].fill(0);
        for i in 0..n_coeffs {
            let pos = self.coeff_pos[c_idx][i] as usize;
            let level = self.coeff_list[c_idx][i];
            self.math_scratchpad[pos] = level;
        }

        if rotate_coeffs {
            self.rotate_coefficients_4x4();
        }

        // --- 3. Transformation Bypass ---
        let mut residual = [0i16; 1024];

        match rdpcm_mode {
            1 => self.apply_rdpcm_horizontal(&mut residual, n_t),
            2 => self.apply_rdpcm_vertical(&mut residual, n_t),
            _ => {
                for i in 0..(n_t * n_t) {
                    residual[i] = self.math_scratchpad[i];
                }
            }
        }

        // --- 4. Cross-Component Prediction ---
        // Use the local 'res_scale' we copied earlier
        if c_idx != 0 && res_scale != 0 {
            self.apply_cross_component_prediction(n_t, res_scale, Some(&mut residual));
        }

        // --- 5. Final Reconstruction ---
        let bit_depth =
            if c_idx == 0 { self.sps.bit_depth_luma } else { self.sps.bit_depth_chroma };
        self.add_residual_and_write(x_t, y_t, n_t, c_idx, Some(&residual), bit_depth);

        if rotate_coeffs {
            self.math_scratchpad[..16].fill(0);
        }
    }
    pub fn apply_rdpcm_horizontal(&self, residual: &mut [i16], n_t: usize) {
        // Accumulates left-to-right across each row
        for y in 0..n_t {
            let mut sum = 0i32; // Safe 32-bit accumulator
            for x in 0..n_t {
                sum += i32::from(self.math_scratchpad[y * n_t + x]);
                let clamped = sum.clamp(-32768, 32767) as i16;
                residual[y * n_t + x] = clamped;
                sum = i32::from(clamped); // Carry clamped value to the next pixel
            }
        }
    }

    pub fn apply_rdpcm_vertical(&self, residual: &mut [i16], n_t: usize) {
        // Accumulates top-to-bottom down each column
        for x in 0..n_t {
            let mut sum = 0i32; // Safe 32-bit accumulator
            for y in 0..n_t {
                // Notice y is on the inner loop, jumping by n_t
                sum += i32::from(self.math_scratchpad[y * n_t + x]);
                let clamped = sum.clamp(-32768, 32767) as i16;
                residual[y * n_t + x] = clamped;
                sum = i32::from(clamped); // Carry clamped value to the next pixel
            }
        }
    }

    pub fn apply_rdpcm_horizontal_in_place(&mut self, n_t: usize) {
        for y in 0..n_t {
            let mut sum = 0i32; // Accumulate in i32
            for x in 0..n_t {
                let idx = y * n_t + x;
                sum += i32::from(self.math_scratchpad[idx]);

                let clamped = sum.clamp(-32768, 32767) as i16;
                self.math_scratchpad[idx] = clamped; // Write back in-place
                sum = i32::from(clamped);
            }
        }
    }

    pub fn apply_rdpcm_vertical_in_place(&mut self, n_t: usize) {
        for x in 0..n_t {
            let mut sum = 0i32; // Accumulate in i32
            for y in 0..n_t {
                let idx = y * n_t + x;
                sum += i32::from(self.math_scratchpad[idx]);

                let clamped = sum.clamp(-32768, 32767) as i16;
                self.math_scratchpad[idx] = clamped; // Write back in-place
                sum = i32::from(clamped);
            }
        }
    }
    pub fn add_residual_and_write(
        &mut self, x0: usize, y0: usize, n_t: usize, c_idx: usize, residual: Option<&[i16]>,
        bit_depth: u8
    ) {
        let residual = residual.unwrap_or(&self.math_scratchpad);

        let mut plane = match c_idx {
            0 => self.raw_frame.luma.lock().unwrap(),
            1 => self.raw_frame.cb.lock().unwrap(),
            2 => self.raw_frame.cr.lock().unwrap(),
            _ => panic!("Invalid component index")
        };

        write_block_and_pad(
            &mut plane,
            x0,
            y0,
            n_t,
            &self.pixel_scratchpad,
            Some(residual),
            bit_depth
        );
    }
    pub fn apply_cross_component_prediction(
        &mut self,
        n_t_c: usize, // Chroma TU size
        res_scale_val: i8,
        residual: Option<&mut [i16]>
    ) {
        if res_scale_val == 0 {
            return;
        }
        let residual = residual.unwrap_or(&mut self.math_scratchpad);

        // Bit depth alignment (usually 0 if Luma and Chroma have same bit depth)
        let bit_depth_luma = self.sps.bit_depth_luma;
        let bit_depth_chroma = self.sps.bit_depth_chroma;
        let shift = i32::from(bit_depth_chroma) - i32::from(bit_depth_luma);

        let chroma_format = self.sps.chroma_format;

        for y in 0..n_t_c {
            for x in 0..n_t_c {
                // Determine which Luma pixel corresponds to this Chroma pixel
                let luma_idx = match chroma_format {
                    ChromaFormat::Yuv420 => {
                        // In 4:2:0, Chroma is half-size.
                        // We map Chroma(x,y) to Luma(2x, 2y)
                        let n_t_l = n_t_c * 2;
                        (y * 2) * n_t_l + (x * 2)
                    }
                    ChromaFormat::Yuv422 => {
                        // In 4:2:2, Chroma is half-width but full-height
                        let n_t_l = n_t_c * 2;
                        y * n_t_l + (x * 2)
                    }
                    ChromaFormat::Yuv444 => {
                        // In 4:4:4, it's 1-to-1
                        y * n_t_c + x
                    }
                    _ => unreachable!()
                };

                let luma_res = self.luma_residual_temp[luma_idx];

                // formula: chroma_res += (scale * (luma_res << shift)) >> 3
                let adjustment = (i32::from(res_scale_val) * (i32::from(luma_res) << shift)) >> 3;
                let current = i32::from(residual[y * n_t_c + x]);
                residual[y * n_t_c + x] = (current + adjustment).clamp(-32768, 32767) as i16;
            }
        }
    }
}

impl DecodeSliceContext<'_> {
    pub fn setup_reference_samples(
        &mut self, x0: usize, y0: usize, n_t: usize, intra_mode: u8, c_idx: usize
    ) -> usize {
        let p_len = 4 * n_t + 1;

        // 1. Reset/Clear internal buffers
        self.ref_samples_p[..p_len].fill(0);
        self.ref_samples_available[..p_len].fill(false);

        // 2. Check Availability (Using PIXEL coordinates x0, y0)
        if true {
            check_availability(
                self.neighbor_tracker,
                x0,
                y0,
                n_t,
                c_idx,
                &mut self.ref_samples_available[..p_len],
                self.sps.pic_width_in_luma_samples as isize,
                self.sps.pic_height_in_luma_samples as isize,
                1 << self.sps.log2_ctb_size_y
            );
        } else {
            check_availability_old(
                self.neighbor_tracker,
                x0,
                y0,
                n_t,
                c_idx,
                &mut self.ref_samples_available
            );
        }
        if DEBUG_MORE {
            println!("--- Reference Border (N={n_t}) ---");
            print_available(&self.ref_samples_available[..p_len], n_t);
        }

        // 3. Fetch pixels from frame and perform HEVC propagation padding
        perform_padding(
            &self.raw_frame,
            &mut self.ref_samples_p[..p_len],
            &self.ref_samples_available[..p_len],
            x0,
            y0,
            n_t,
            c_idx
        );
        if DEBUG_MORE {
            println!("--- Reference Border (N={n_t}) ---");
            print_border(&self.ref_samples_p[..p_len], n_t);
        }

        if c_idx == 0 {
            // 4. Apply Smoothing filters (Standard [1,2,1] or Strong 32x32)
            let strong_enabled = self.sps.strong_intra_smoothing_enable_flag;
            apply_reference_smoothing(
                &mut self.ref_samples_p[..p_len],
                n_t,
                intra_mode,
                strong_enabled
            );
        }

        p_len
    }
}

fn write_block_and_pad(
    plane: &mut SingleFrame, x0: usize, y0: usize, n_t: usize, pred: &[u8],
    residual: Option<&[i16]>, bit_depth: u8
) {
    let max_val = (1_i32 << bit_depth) - 1;
    let buf = &mut plane.pixels;
    let (w, h, s, p) = (plane.width, plane.height, plane.stride, plane.padding);

    let x_end = x0 + n_t;
    let y_end = y0 + n_t;
    let frame_ox = p;
    let frame_oy = p;

    if let Some(b) = residual {
        // --- 1. Reconstruct directly into the padded buffer ---
        //
        // for dy in 0..n_t {
        //     let dst_row = (frame_oy + y0 + dy) * s + (frame_ox + x0);
        //     let i_start = dy * n_t;
        //     for dx in 0..n_t {
        //         let i = i_start + dx;
        //         buf[dst_row + dx] = (b[i] + pred[i] as i32).clamp(0, max_val) as u8;
        //     }
        // }
        for (dy, (b_row, pred_row)) in b.chunks(n_t).zip(pred.chunks(n_t)).take(n_t).enumerate() {
            let dst_row = (frame_oy + y0 + dy) * s + (frame_ox + x0);
            let dst_slice = &mut buf[dst_row..dst_row + n_t];

            for (dst, (&bv, &pv)) in dst_slice.iter_mut().zip(b_row.iter().zip(pred_row)) {
                let sum = i32::from(bv) + i32::from(pv);
                if DEBUG_MORE && sum > 255 {
                    println!("CLIPPING DETECTED: Pred={pv} + Residual={bv} = {sum}");
                }
                *dst = sum.clamp(0, max_val) as u8;
            }
        }
    } else {
        // just copy-paste residual into the buffer
        for dy in 0..n_t {
            let dst_row = (frame_oy + y0 + dy) * s + (frame_ox + x0);
            buf[dst_row..dst_row + n_t].copy_from_slice(&pred[dy * n_t..(dy + 1) * n_t]);
        }
    }

    if DEBUG_MORE {
        println!("--- Out Padding (N={n_t}) ---");
        for dy in 0..n_t {
            let dst_row = (frame_oy + y0 + dy) * s + (frame_ox + x0);
            for dx in 0..n_t {
                print!("{} ", buf[dst_row + dx]);
            }
            println!();
        }
    }
    // --- 2. Left edge ---
    if x0 == 0 {
        for dy in 0..n_t {
            let row = (frame_oy + y0 + dy) * s + frame_ox;
            let val = buf[row];
            buf[row - p..row].fill(val);
        }
    }

    // --- 3. Right edge ---
    if x_end == w {
        for dy in 0..n_t {
            let row_last = (frame_oy + y0 + dy) * s + frame_ox + w - 1;
            let val = buf[row_last];
            buf[row_last + 1..row_last + 1 + p].fill(val);
        }
    }

    // --- 4. Top edge ---
    if y0 == 0 {
        let src_row_base = frame_oy * s;
        let x_start = if x0 == 0 { 0 } else { frame_ox + x0 };
        let x_stop = if x_end == w { s } else { frame_ox + x_end };
        for py in 1..=p {
            buf.copy_within(
                src_row_base + x_start..src_row_base + x_stop,
                src_row_base - py * s + x_start
            );
        }
    }

    // --- 5. Bottom edge ---
    if y_end == h {
        let src_row_base = (frame_oy + h - 1) * s;
        let x_start = if x0 == 0 { 0 } else { frame_ox + x0 };
        let x_stop = if x_end == w { s } else { frame_ox + x_end };
        for py in 1..=p {
            buf.copy_within(
                src_row_base + x_start..src_row_base + x_stop,
                src_row_base + py * s + x_start
            );
        }
    }
}
pub fn print_available(available: &[bool], n_t: usize) {
    let total = 4 * n_t;

    // We loop strictly forward through the linear 0..4N array
    for i in 0..=total {
        print!("{}", u8::from(available[i]));

        // Print libde265-style separators at the segment boundaries
        if i == n_t - 1 || i == 2 * n_t - 1 || i == 2 * n_t || i == 3 * n_t {
            println!("|");
        } else if i != total {
            print!(" ");
        }
    }
    println!();
}

pub fn print_border(p: &[u8], n_t: usize) {
    let total = 4 * n_t;

    // Exact same linear sweep for the values
    for i in 0..=total {
        print!("{}", p[i]);

        if i == n_t - 1 || i == 2 * n_t - 1 || i == 2 * n_t || i == 3 * n_t {
            println!("|");
        } else if i != total {
            print!(" ");
        }
    }
    println!();
}

fn check_availability_old(
    tracker: &NeighborTracker, x0: usize, y0: usize, n_t: usize, c_idx: usize,
    available: &mut [bool]
) {
    // 4:2:0 Scaling: Chroma pixels (c_idx 1,2) correspond to 2x2 Luma areas

    let scale = if c_idx == 0 { 1 } else { 2 };

    let sx0 = x0 * scale;

    let sy0 = y0 * scale;

    // 1. Indices 0 to 2*nT - 1: Below-Left and Left (Bottom-to-Top)

    // We start from the very bottom neighbor and move UP toward the corner

    for i in 0..(2 * n_t) {
        let px = sx0 as isize - 1; // Safely becomes -1 at the left edge

        // i=0 is the bottom-most pixel: (y0 + 2*nT - 1)

        let py = sy0 as isize + ((2 * n_t - 1 - i) * scale) as isize;

        available[i] = tracker.is_available(sx0, sy0, px, py);
    }

    // 2. Index 2*nT: Top-Left Corner

    available[2 * n_t] = tracker.is_available(
        sx0,
        sy0,
        sx0 as isize - 1, // Safely becomes -1
        sy0 as isize - 1  // Safely becomes -1
    );

    // 3. Indices 2*nT + 1 to 4*nT: Top and Top-Right (Left-to-Right)

    for i in 1..=(2 * n_t) {
        // i=1 is directly above x0, i=2nT is Top-Right

        let px = sx0 as isize + ((i - 1) * scale) as isize;

        let py = sy0 as isize - 1; // Safely becomes -1 at the top edge

        available[2 * n_t + i] = tracker.is_available(sx0, sy0, px, py);
    }
}
#[allow(clippy::too_many_arguments)]
fn check_availability(
    tracker: &NeighborTracker, x0: usize, y0: usize, n_t: usize, c_idx: usize,
    available: &mut [bool], frame_width: isize, frame_height: isize, ctu_size: isize
) {
    let scale = if c_idx == 0 { 1 } else { 2 };
    let sx0 = (x0 * scale) as isize;
    let sy0 = (y0 * scale) as isize;

    // HEVC neighbor availability operates on a 4x4 luma grid minimum.
    // We scale this chunk size for chroma.
    let chunk_size = 4 / scale;

    // --- 1. Below-Left and Left (Bottom-to-Top) ---
    // Process in chunks rather than pixel-by-pixel
    let mut i = 0;
    while i < 2 * n_t {
        let px = sx0 - 1;
        let py = sy0 + ((2 * n_t - 1 - i) * scale) as isize;

        // Fast Coordinate Assumptions
        let is_avail = if px < 0 || py >= frame_height {
            false
        } else if py >= ((sy0 / ctu_size) + 1) * ctu_size {
            // CTU Rule: If py crosses into the CTU directly below, it's not decoded yet.
            false
        } else {
            tracker.is_available(sx0 as usize, sy0 as usize, px, py)
        };

        // Fill the chunk
        for j in 0..chunk_size {
            if i + j < 2 * n_t {
                available[i + j] = is_avail;
            }
        }
        i += chunk_size;
    }

    // --- 2. Top-Left Corner ---
    available[2 * n_t] = if sx0 - 1 < 0 || sy0 - 1 < 0 {
        false
    } else {
        tracker.is_available(sx0 as usize, sy0 as usize, sx0 - 1, sy0 - 1)
    };

    // --- 3. Top and Top-Right (Left-to-Right) ---
    let mut i = 1;
    while i <= 2 * n_t {
        let px = sx0 + ((i - 1) * scale) as isize;
        let py = sy0 - 1;

        let is_avail = if py < 0 || px >= frame_width {
            false
        } else if py >= (sy0 / ctu_size) * ctu_size && px >= ((sx0 / ctu_size) + 1) * ctu_size {
            // CTU Rule: If py is in the current CTU row, but px crosses into the next CTU right, it's false.
            false
        } else {
            tracker.is_available(sx0 as usize, sy0 as usize, px, py)
        };

        // Fill the chunk
        for j in 0..chunk_size {
            if i + j <= 2 * n_t {
                available[2 * n_t + i + j] = is_avail;
            }
        }
        i += chunk_size;
    }
}

fn perform_padding(
    frame: &Arc<RawFrame>, p: &mut [u8], available: &[bool], x0: usize, y0: usize, n_t: usize,
    c_idx: usize
) {
    let total = 4 * n_t + 1;
    let bit_depth = 8;

    // 1. Fetch available pixels into the strictly linear 0..4N array
    {
        let plane = match c_idx {
            0 => frame.luma.lock().unwrap(),
            1 => frame.cb.lock().unwrap(),
            2 => frame.cr.lock().unwrap(),
            _ => unreachable!()
        };
        let (pixels, stride, pad) = (plane.pixels.as_slice(), plane.stride, plane.padding);

        // SAFE GET_P: Add pad as isize FIRST to prevent usize::MAX overflow
        let pad_i = pad as isize;
        let get_p = |px: isize, py: isize| {
            pixels[((py + pad_i) as usize) * stride + ((px + pad_i) as usize)]
        };

        for i in 0..total {
            if available[i] {
                // Map the 1D linear index 'i' back to 2D image coordinates
                let (px, py) = if i < 2 * n_t {
                    // Indices 0 to 2*nT - 1: Below-Left and Left
                    (x0 as isize - 1, y0 as isize + (2 * n_t - 1 - i) as isize)
                } else if i == 2 * n_t {
                    // Index 2*nT: Top-Left Corner
                    (x0 as isize - 1, y0 as isize - 1)
                } else {
                    // Indices 2*nT + 1 to 4*nT: Top and Top-Right
                    (x0 as isize + (i - 2 * n_t - 1) as isize, y0 as isize - 1)
                };
                debug_more!("px={}, py={},v={}", px, py, get_p(px, py));

                p[i] = get_p(px, py);
            }
        }
    }

    // 2. HEVC Reference Sample Substitution (Spec 8.4.4.2.2)
    let n_avail = available.iter().filter(|&&a| a).count();

    if n_avail == 0 {
        // Case 1: No samples available at all -> Fill with mid-grey
        p.fill(1 << (bit_depth - 1));
    } else if n_avail < total {
        // Case 2: Partial availability -> Substitution sweep

        // Find the very first available sample starting from the bottom-left
        let first_idx = available.iter().position(|&a| a).unwrap();
        let first_value = p[first_idx];

        // Backfill: If the first available sample isn't at index 0, fill backwards
        for i in 0..first_idx {
            p[i] = first_value;
        }

        // Forward fill: Propagate the previous valid value into any remaining gaps
        for i in (first_idx + 1)..total {
            if !available[i] {
                p[i] = p[i - 1];
            }
        }
    }
}
fn apply_reference_smoothing(p: &mut [u8], n_t: usize, mode: u8, strong_enabled: bool) {
    if n_t == 4 {
        return;
    }

    if n_t == 32 && strong_enabled {
        let threshold = 1 << (8 - 5); // Assuming 8-bit depth (BitDepth - 5)

        let bl = i32::from(p[0]); // Bottom-Left
        let mid_l = i32::from(p[n_t]); // Mid-Left
        let tl = i32::from(p[2 * n_t]); // Top-Left Corner
        let mid_t = i32::from(p[3 * n_t]); // Mid-Top
        let tr = i32::from(p[4 * n_t]); // Top-Right

        // Spec 8.4.4.2.3: Strong smoothing condition checks the left edge and top edge
        if (bl + tl - 2 * mid_l).abs() < threshold && (tl + tr - 2 * mid_t).abs() < threshold {
            apply_strong_smoothing(p, n_t);
            return;
        }
    }

    if is_filtering_required(mode, n_t) {
        // Store the unmodified p[0] before the loop begins
        let mut prev = p[0];

        // Sweep perfectly across the perimeter without allocating a copy
        for i in 1..(4 * n_t) {
            let curr = p[i];
            let next = p[i + 1];

            // Apply the [1, 2, 1] filter using the original `prev` value
            p[i] = ((u16::from(prev) + 2 * u16::from(curr) + u16::from(next) + 2) >> 2) as u8;

            // The unmodified `curr` becomes the `prev` for the next iteration
            prev = curr;
        }
    }
}

fn apply_strong_smoothing(p: &mut [u8], n_t: usize) {
    let bl = i32::from(p[0]); // Index 0: Bottom-Left
    let tl = i32::from(p[2 * n_t]); // Index 2N: Top-Left Corner
    let tr = i32::from(p[4 * n_t]); // Index 4N: Top-Right

    let n2 = 2 * n_t; // 2N distance

    // 1. Left column (Interpolating between Bottom-Left at 0 and Top-Left at 2N)
    for i in 1..n2 {
        // Distance from BL is i. Distance from TL is (2N - i).
        p[i] = (((n2 - i) as i32 * bl + (i as i32) * tl + n_t as i32) / n2 as i32) as u8;
    }

    // 2. Top row (Interpolating between Top-Left at 2N and Top-Right at 4N)
    for i in 1..n2 {
        let idx = n2 + i;
        // Distance from TL is (2N - i). Distance from TR is i.
        p[idx] = (((n2 - i) as i32 * tl + (i as i32) * tr + n_t as i32) / n2 as i32) as u8;
    }
}

fn is_filtering_required(mode: u8, n_t: usize) -> bool {
    // HEVC Table 8-3
    match n_t {
        8 => mode == 0 || mode == 2 || mode == 18 || mode == 34,
        16 => mode != 1 && mode != 10 && mode != 26,
        32 => mode != 1 && mode != 10 && mode != 26,
        _ => false
    }
}
