use std::sync::Arc;

use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac::CabacDecoder;
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, Pps, SliceHeader, Sps};
use crate::hevc_decoder::neighbor_tracker::NeighborTracker;
use crate::hevc_decoder::quadtree::sao::SaoInfo;
use crate::hevc_decoder::quadtree::sig_ctx_generator::generate_all_sig_ctx_maps;
use crate::hevc_decoder::raw_frame::RawFrame;

pub struct DecodeSliceContext<'a> {
    pub sps:                  &'a Sps,
    pub pps:                  &'a Pps,
    pub slice_header:         &'a SliceHeader,
    pub cabac:                CabacDecoder<'a>,
    pub neighbor_tracker:     &'a mut NeighborTracker,
    pub is_cu_qp_delta_coded: bool,
    pub cu_qp_delta:          i32,
    // quantization group
    pub current_qg_x:         usize,
    pub current_qg_y:         usize,
    pub last_qp_in_slice:     i8, // This tracks the "previous" QP for the next CU

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
    pub math_scratchpad:           Vec<i32>,
    // Reference Wall buffers
    pub ref_samples_p:             Vec<u8>,
    pub ref_samples_available:     Vec<bool>,
    pub ref_main_buf:              Vec<u8>,
    pub res_scale_val:             i8,
    ///  Stores the Luma residuals for the current TU area
    /// so Chroma can use them for CCP.
    pub luma_residual_temp:        Vec<i32>,

    pub ctb_sao_buffer: Vec<SaoInfo>
}
impl<'a> DecodeSliceContext<'a> {
    pub fn new(
        sps: &'a Sps, pps: &'a Pps, slice_header: &'a SliceHeader, cabac_engine: CabacDecoder<'a>,
        neighbor_tracker: &'a mut NeighborTracker, last_qp_in_slice: i8, raw_frame: Arc<RawFrame>
    ) -> Self {
        // SAO data

        let ctb_size = 1 << sps.log2_ctb_size_y;

        let width_in_ctbs = (sps.pic_width_in_luma_samples + ctb_size - 1) / ctb_size;
        let height_in_ctbs = (sps.pic_height_in_luma_samples + ctb_size - 1) / ctb_size;

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
            current_qg_x: 0,
            current_qg_y: 0,
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
            res_scale_val: -1,
            ctb_sao_buffer: vec![SaoInfo::default(); buffer_size]
        }
    }
}

impl<'a> DecodeSliceContext<'a> {
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

    #[inline]
    fn get_ctb_addr_rs(&self, x: usize, y: usize) -> usize {
        y * (self.sps.pic_width_in_ctbs_y as usize) + x
    }

    #[inline]
    fn get_tile_id(&self, x: usize, y: usize) -> u16 {
        // Accessing the PPS tile map
        let addr = self.get_ctb_addr_rs(x, y);
        self.pps.tile_id_rs[addr]
    }
}
impl<'a> DecodeSliceContext<'a> {
    /// Sets a block of pixels (e.g., after reconstruction)
    ///
    /// Data is expected to be in scratchpad
    pub fn write_block_scratchpad(&self, c_idx: usize, x0: usize, y0: usize, n_t: usize) {
        // data is expected to be in scratchpad
        let block_data = &self.pixel_scratchpad;
        // 1. Lock the appropriate plane
        let mut plane = match c_idx {
            0 => self.raw_frame.luma.lock().unwrap(),
            1 => self.raw_frame.cb.lock().unwrap(),
            2 => self.raw_frame.cr.lock().unwrap(),
            _ => panic!("Invalid component index")
        };

        // 2. Calculate coordinates with padding offset
        let stride = plane.stride;
        let padding = plane.padding;
        let offset_base = (y0 + padding) * stride + (x0 + padding);

        // 3. Copy row by row
        for dy in 0..n_t {
            let src_start = dy * n_t;
            let dst_start = offset_base + (dy * stride);

            plane.pixels[dst_start..dst_start + n_t]
                .copy_from_slice(&block_data[src_start..src_start + n_t]);
        }
    }
    pub fn scale_coefficients(
        &mut self,
        xT: usize,
        yT: usize,  // TU pos
        n_t: usize, // TU size (4, 8, 16, 32)
        c_idx: usize,
        transform_skip_flag: bool
    ) {
        debug_more!(
            "scale_coefficients :xT={} yT={} n_t={} cidx={}",
            xT,
            yT,
            n_t,
            c_idx
        );
        const LEVEL_SCALE: [i32; 6] = [40, 45, 51, 57, 64, 72];

        let sps = self.sps;
        let pps = self.pps;

        // 1. Get the QP for this component
        let qp = match c_idx {
            0 => self.qp_y_prime,
            1 => self.qp_cb_prime,
            2 => self.qp_cr_prime,
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
                self.math_scratchpad[pos] = level as i32;
            }
            return;
        }

        // 4. Calculate bdShift (Spec 8.6.3)
        let bit_depth = if c_idx == 0 { sps.bit_depth_luma } else { sps.bit_depth_chroma };
        let log2_n_t = n_t.trailing_zeros() as i32;
        let mut bd_shift = bit_depth as i32 + log2_n_t - 5;

        // 5. Scaling Logic
        if sps.scaling_list_enabled_flag == false {
            // Default Scaling (m_x_y = 16)
            bd_shift -= 4;
            debug_more!("bd_shift:{}", bd_shift);
            let offset = 1 << (bd_shift - 1);
            let fact = LEVEL_SCALE[(qp % 6) as usize] << (qp / 6);

            for i in 0..self.n_coeff[c_idx] {
                let pos = self.coeff_pos[c_idx][i as usize] as usize;
                let level = self.coeff_list[c_idx][i as usize] as i64;

                // The actual scaling math
                let scaled = (level * fact as i64 + offset as i64) >> bd_shift;

                // Clip to 16-bit range
                self.math_scratchpad[pos] = scaled.clamp(-32768, 32767) as i32;
            }
        } else {
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
                let level = self.coeff_list[c_idx][i as usize] as i64;

                // --- FIX: Map NxN position to 8x8 scaling list ---
                let mut m_x_y = if n_t <= 8 {
                    scaling_list[pos] as i64
                } else {
                    let x = pos % n_t;
                    let y = pos / n_t;
                    let ratio = n_t >> 3; // 2 for 16x16, 4 for 32x32
                    scaling_list[(y / ratio) * 8 + (x / ratio)] as i64
                };

                // Special Case: DC Coefficient (Spec 8.6.3)
                if pos == 0 {
                    if n_t == 16 {
                        m_x_y = pps.pic_scaling_lists.dc16[matrix_id] as i64;
                    } else if n_t == 32 {
                        m_x_y = pps.pic_scaling_lists.dc32[matrix_id] as i64;
                    }
                }

                let fact = (m_x_y * LEVEL_SCALE[(qp % 6) as usize] as i64) << (qp / 6);
                let scaled = (level * fact + offset as i64) >> bd_shift;

                let final_clipped = scaled.clamp(-32768, 32767);
                if DEBUG_MORE {
                    println!(
                        "TRACE_SCALE: i={:>2} pos={:>4} level={:>4} m_x_y={:>3} fact={:>8} bdShift={:>2} final={:>5}",
                        i, pos, level, m_x_y, fact, bd_shift, final_clipped
                    );
                }

                self.math_scratchpad[pos] = final_clipped as i32;
            }
        }
        // --- do transform or skip ---

        // Note: We only print if n_t is 4 or 8 to prevent overwhelming the console.
        // In a real debug session, you might remove this check.
        if DEBUG_MORE {
            if n_t <= 32 {
                println!(
                    "coefficients OUT (cIdx:{} at {},{} size:{}):",
                    c_idx, xT, yT, n_t
                );
                for y in 0..n_t {
                    print!("  ");
                    for x in 0..n_t {
                        // In your Rust port, coeffStride is just n_t since coeff_buffer is 1D
                        let val = self.math_scratchpad[y * n_t + x];
                        print!("{:3} ", val);
                    }
                    println!();
                }
            }
        }
    }
    /// Returns a slice of the scaling factors (m[x][y]) for the current TU.
    pub fn get_scaling_list(&self, n_t: usize, c_idx: usize, is_intra: bool) -> &[u8] {
        let pps = &self.pps;

        // 1. Determine MatrixID based on libde265 logic
        let mut matrix_id = c_idx;

        if n_t == 32 {
            // 32x32 only has Luma IDs (0 for Intra, 1 for Inter)
            matrix_id = 0;
            if !is_intra {
                matrix_id = 1;
            }
        } else {
            // 4x4, 8x8, 16x16 have 3 Intra followed by 3 Inter matrices
            if !is_intra {
                matrix_id += 3;
            }
        }

        // 2. Return the correct buffer based on size
        match n_t {
            4 => &pps.pic_scaling_lists.size0[matrix_id], // [6][16]
            8 => &pps.pic_scaling_lists.size1[matrix_id], // [6][64]
            16 => &pps.pic_scaling_lists.size2[matrix_id], // [6][256]
            32 => &pps.pic_scaling_lists.size3[matrix_id], // [2][1024]
            _ => unreachable!("Invalid TU size for scaling list")
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
            .map_or(false, |rext| rext.transform_skip_rotation_enabled_flag);

        let rotate_coeffs = transform_skip_rotation_enabled && n_t == 4 && self.is_intra;

        // --- 2. Scatter coefficients ---
        // Access math_scratchpad directly
        self.math_scratchpad[..n_t * n_t].fill(0);
        for i in 0..n_coeffs {
            let pos = self.coeff_pos[c_idx][i] as usize;
            let level = self.coeff_list[c_idx][i];
            self.math_scratchpad[pos] = level as i32;
        }

        if rotate_coeffs {
            self.rotate_coefficients_4x4();
        }

        // --- 3. Transformation Bypass ---
        let mut residual = [0i32; 1024];

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
    pub fn apply_rdpcm_horizontal(&self, residual: &mut [i32], nT: usize) {
        for y in 0..nT {
            let mut sum = 0i32;
            for x in 0..nT {
                sum += self.math_scratchpad[y * nT + x] as i32;
                residual[y * nT + x] = sum;
            }
        }
    }

    pub fn apply_rdpcm_vertical(&self, residual: &mut [i32], nT: usize) {
        for x in 0..nT {
            let mut sum = 0i32;
            for y in 0..nT {
                sum += self.math_scratchpad[y * nT + x] as i32;
                residual[y * nT + x] = sum;
            }
        }
    }
    pub fn add_residual_and_write(
        &mut self, x0: usize, y0: usize, n_t: usize, c_idx: usize, residual: Option<&[i32]>,
        bit_depth: u8
    ) {
        let residual = residual.unwrap_or(&self.math_scratchpad);
        let max_val = ((1i32 << bit_depth) - 1) as i32;

        // 1. Lock the appropriate plane
        let mut plane = match c_idx {
            0 => self.raw_frame.luma.lock().unwrap(),
            1 => self.raw_frame.cb.lock().unwrap(),
            2 => self.raw_frame.cr.lock().unwrap(),
            _ => panic!("Invalid component index")
        };

        let stride = plane.stride;
        let padding = plane.padding;
        let offset_base = (y0 + padding) * stride + (x0 + padding);

        // 2. Process row by row
        for dy in 0..n_t {
            let start_idx = dy * n_t;
            let end_idx = start_idx + n_t;

            // Slices for the current row
            let pred_row = &self.pixel_scratchpad[start_idx..end_idx];
            let res_row = &residual[start_idx..end_idx];

            let dst_offset = offset_base + (dy * stride);
            let dst_row = &mut plane.pixels[dst_offset..dst_offset + n_t];

            // 3. Zip prediction and residual, add, clamp, and write to destination
            for x in 0..n_t {
                let p = pred_row[x] as i32;
                let r = res_row[x];

                // Pixel = Clip3(0, max_val, Pred + Res)
                let output = (p + r).clamp(0, max_val) as u8;
                dst_row[x] = output;

                if DEBUG_MORE {
                    print!("{:3} ", output);
                }
            }
            if DEBUG_MORE {
                println!();
            }
        }
    }
    pub fn apply_cross_component_prediction(
        &mut self,
        n_t_c: usize, // Chroma TU size
        res_scale_val: i8,
        residual: Option<&mut [i32]>
    ) {
        if res_scale_val == 0 {
            return;
        }
        let residual = residual.unwrap_or(&mut self.math_scratchpad);

        // Bit depth alignment (usually 0 if Luma and Chroma have same bit depth)
        let bit_depth_luma = self.sps.bit_depth_luma;
        let bit_depth_chroma = self.sps.bit_depth_chroma;
        let shift = bit_depth_chroma as i32 - bit_depth_luma as i32;

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
                let adjustment = (res_scale_val as i32 * (luma_res << shift)) >> 3;
                residual[y * n_t_c + x] += adjustment;
            }
        }
    }
}

impl<'a> DecodeSliceContext<'a> {
    /// Prepares the reference samples in the context's internal buffers.
    /// Returns the length of the valid segment (4 * n_t + 1).
    pub fn setup_reference_samples(
        &mut self, x0: usize, y0: usize, n_t: usize, intra_mode: u8, c_idx: usize
    ) -> usize {
        let p_len = 4 * n_t + 1;

        // 1. Reset/Clear the values for the current segment
        // We only clear up to p_len to save cycles
        self.ref_samples_p[..p_len].fill(0);
        self.ref_samples_available[..p_len].fill(false);

        // 2. Check Availability
        // We pass slices of our fixed arrays
        check_availability(
            &self.neighbor_tracker,
            &self.pps,
            x0 / n_t,
            y0 / n_t,
            n_t,
            &mut self.ref_samples_available[..p_len]
        );

        // 3. Fetch and Pad
        perform_padding(
            &self.raw_frame,
            &mut self.ref_samples_p[..p_len],
            &self.ref_samples_available[..p_len],
            x0,
            y0,
            n_t,
            c_idx
        );

        // 4. Filter/Smoothing
        let strong_enabled = self.sps.strong_intra_smoothing_enable_flag;
        apply_reference_smoothing(
            &mut self.ref_samples_p[..p_len],
            n_t,
            intra_mode,
            strong_enabled
        );

        p_len
    }
}

fn check_availability(
    tracker: &NeighborTracker, pps: &Pps, x0: usize, y0: usize, n_t: usize, available: &mut [bool]
) {
    // Top-Left
    available[0] = tracker.is_available(x0, y0, x0 as isize - 1, y0 as isize - 1);

    // Top & Top-Right
    for i in 0..(2 * n_t) {
        available[1 + i] = tracker.is_available(x0, y0, (x0 + i) as isize, y0 as isize - 1);
    }

    // Left & Below-Left
    for i in 0..(2 * n_t) {
        available[1 + 2 * n_t + i] =
            tracker.is_available(x0, y0, x0 as isize - 1, (y0 + i) as isize);
    }
    if DEBUG_MORE {
        debug_more!("available \n");
        available.chunks(n_t).for_each(|chunk| {
            println!("{:?}", chunk);
        })
    }

    if pps.constrained_intra_pred_flag {
        // filter_constrained logic...
        filter_constrained(tracker, x0, y0, n_t, available);
    }
}

fn perform_padding(
    frame: &Arc<RawFrame>, p: &mut [u8], available: &[bool], x0: usize, y0: usize, n_t: usize,
    c_idx: usize
) {
    // 1. Fetch available pixels from the Mutex-protected frame
    {
        let plane = match c_idx {
            0 => frame.luma.lock().unwrap(),
            1 => frame.cb.lock().unwrap(),
            2 => frame.cr.lock().unwrap(),
            _ => unreachable!()
        };

        let stride = plane.stride;
        let pad = plane.padding;
        let pixels = &plane.pixels;

        let get_p = |px: isize, py: isize| -> u8 {
            pixels[(py as usize + pad) * stride + (px as usize + pad)]
        };

        if available[0] {
            p[0] = get_p(x0 as isize - 1, y0 as isize - 1);
        }
        for i in 0..(2 * n_t) {
            if available[1 + i] {
                p[1 + i] = get_p((x0 + i) as isize, y0 as isize - 1);
            }
            if available[1 + 2 * n_t + i] {
                p[1 + 2 * n_t + i] = get_p(x0 as isize - 1, (y0 + i) as isize);
            }
        }
    }

    // 2. Propagation Logic (HEVC Spec 8.4.4.2.2)
    if !available.iter().any(|&a| a) {
        p.fill(128); // Default gray if nothing is available
        return;
    }

    // Standard HEVC search order: Bottom-Left -> Corner -> Top-Right
    let mut order = Vec::with_capacity(p.len());
    for i in ((2 * n_t + 1)..=(4 * n_t)).rev() {
        order.push(i);
    }
    order.push(0);
    for i in 1..=(2 * n_t) {
        order.push(i);
    }

    let first_valid_idx = *order.iter().find(|&&idx| available[idx]).unwrap();
    let mut last_val = p[first_valid_idx];

    for &idx in &order {
        if available[idx] {
            last_val = p[idx];
        } else {
            p[idx] = last_val;
        }
    }
}
/// Applies [1, 2, 1] smoothing or Strong Intra Smoothing (Spec 8.4.4.2.3)
fn apply_reference_smoothing(p: &mut [u8], n_t: usize, mode: u8, strong_enabled: bool) {
    if n_t == 4 {
        return;
    } // 4x4 is never smoothed

    // Strong Intra Smoothing check for 32x32 blocks
    if n_t == 32 && strong_enabled {
        let threshold = 1 << (8 - 5); // Default for 8-bit
        let tl = p[0] as i32;
        let tr = p[2 * n_t] as i32;
        let bl = p[4 * n_t] as i32;

        if (tl + tr - 2 * p[n_t] as i32).abs() < threshold
            && (tl + bl - 2 * p[3 * n_t] as i32).abs() < threshold
        {
            apply_strong_smoothing(p, n_t);
            return;
        }
    }

    // Standard [1, 2, 1] smoothing
    if is_filtering_required(mode, n_t) {
        let mut p_copy = p.to_vec();
        for i in 1..4 * n_t {
            p_copy[i] = ((p[i - 1] as u16 + 2 * p[i] as u16 + p[i + 1] as u16 + 2) >> 2) as u8;
        }
        // Corner and ends are not smoothed or use specific rules;
        // standard HEVC logic skips p[0] and p[4*nT] during standard filter.
        p.copy_from_slice(&p_copy);
    }
}
fn apply_strong_smoothing(p: &mut [u8], n_t: usize) {
    let tl = p[0] as i32;
    let tr = p[2 * n_t] as i32;
    let bl = p[4 * n_t] as i32;

    // Top edge bilinear interpolation
    for i in 1..=2 * n_t {
        p[i] = (((2 * n_t - i) as i32 * tl + i as i32 * tr + n_t as i32) / (2 * n_t) as i32) as u8;
    }
    // Left edge bilinear interpolation
    for i in 1..=2 * n_t {
        p[2 * n_t + i] =
            (((2 * n_t - i) as i32 * tl + i as i32 * bl + n_t as i32) / (2 * n_t) as i32) as u8;
    }
}

fn filter_constrained(
    tracker: &NeighborTracker, x0: usize, y0: usize, n_t: usize, available: &mut [bool]
) {
    let mut check = |px: usize, py: usize, idx: usize| {
        if available[idx] && !tracker.get_state(px, py).is_intra {
            available[idx] = false;
        }
    };

    check(x0 - 1, y0 - 1, 0);
    for i in 0..(2 * n_t) {
        check(x0 + i, y0 - 1, 1 + i);
        check(x0 - 1, y0 + i, 1 + 2 * n_t + i);
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
