use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_CBF_CHROMA, CONTEXT_MODEL_CBF_LUMA, CONTEXT_MODEL_LOG2_RES_SCALE_ABS_PLUS1,
    CONTEXT_MODEL_RES_SCALE_SIGN_FLAG, CONTEXT_MODEL_SPLIT_TRANSFORM_FLAG
};
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::idct::{
    idct_4x4_hevc, idct_8x8_hevc, idct_16x16_hevc, idct_32x32_hevc, idst_4x4_hevc
};
use crate::hevc_decoder::nal_parser::NalError;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::neighbor_tracker::PredMode;
use crate::hevc_decoder::quadtree::intra_prediction::decode_intra_prediction;
use crate::hevc_decoder::quadtree::quant::{decode_cu_qp_delta, decode_quantization_parameters};
use crate::hevc_decoder::quadtree::residual_block::decode_residual_block;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum Component {
    Luma = 0,
    Cb = 1,
    Cr = 2
}

fn decode_cbf_luma(ctx: &mut DecodeSliceContext, trafo_depth: u8) -> bool {
    debug_more!("decode_cbf_luma");
    let ctx_idx = CONTEXT_MODEL_CBF_LUMA + (if trafo_depth == 0 { 1 } else { 0 });
    let bit = ctx.cabac.decode_decision(ctx_idx) == 1;
    debug_more!("  decode_cbf_luma=>{bit}");
    bit
}
fn decode_cbf_chroma(ctx: &mut DecodeSliceContext, trafo_depth: u8) -> u8 {
    debug_more!("decode_cbf_chroma");
    let ctx_idx = CONTEXT_MODEL_CBF_CHROMA + (trafo_depth as usize);
    let bit = ctx.cabac.decode_decision(ctx_idx);
    debug_more!("  decode_cbf_chroma=>{bit}");
    bit
}
fn decode_split_transform_flag(ctx: &mut DecodeSliceContext, log2_trafo_size: u8) -> bool {
    debug_more!(
        "decode_split_transform_flag (log2_trafo_size={})",
        log2_trafo_size
    );
    let context = 5_u8.wrapping_sub(log2_trafo_size) as usize;
    assert!( context <= 2);
    let ctx_idx = CONTEXT_MODEL_SPLIT_TRANSFORM_FLAG + context;
    let flag = ctx.cabac.decode_decision(ctx_idx) == 1;
    debug_more!("  decode_split_transform_flag=>{flag}");

    return flag;
}

pub fn decode_tu(
    ctx: &mut DecodeSliceContext,
    x0: usize,
    y0: usize,
    n_t: usize, // TU size (4, 8, 16, 32)
    c_idx: usize,
    cu_pred_mode: PredMode,
    cbf: bool
) {
    let mut residual_dpcm = 0;
    let sps = ctx.sps;

    // --- 1. INTRA PREDICTION ---
    // Predicted pixels are written to pixel_scratchpad
    if cu_pred_mode == PredMode::ModeIntra {
        let intra_pred_mode = if c_idx == 0 {
            ctx.neighbor_tracker.get_intra_mode(x0, y0)
        } else {
            let sub_width_c = sps.sub_width_c as usize;
            let sub_height_c = sps.sub_height_c as usize;
            // Chroma uses the luma coordinates to fetch its specific mode
            ctx.neighbor_tracker
                .get_intra_mode_chroma(x0 * sub_width_c, y0 * sub_height_c)
        };

        if intra_pred_mode >= 35 {
            panic!("intra_pred_mode cannot be more than 35");
        }
        debug_more!("intra_pred_mode=>{intra_pred_mode} c_idx={c_idx}");

        decode_intra_prediction(ctx, x0, y0, intra_pred_mode, n_t, c_idx);

        // Determine Implicit RDPCM (Spec 8.6.4.2)
        let implicit_rdpcm_enabled = sps
            .range_extension
            .as_ref()
            .map_or(false, |s| s.implicit_rdpcm_enabled_flag);

        if implicit_rdpcm_enabled
            && (ctx.cu_transquant_bypass_flag || ctx.transform_skip_flag[c_idx] == 1)
        {
            if intra_pred_mode == 10 {
                residual_dpcm = 1;
            }
            // Horizontal
            else if intra_pred_mode == 26 {
                residual_dpcm = 2;
            } // Vertical
        }
    } else {
        // Inter logic for Explicit RDPCM
        if ctx.explicit_rdpcm_flag {
            residual_dpcm = if ctx.explicit_rdpcm_dir > 0 { 2 } else { 1 };
        }
    }

    // --- 2. LOSSLESS (TRANSQUANT BYPASS) PATH ---
    if ctx.cu_transquant_bypass_flag {
        // This function handles scattering, RDPCM, CCP, and frame write
        ctx.reconstruct_lossless(x0, y0, n_t, c_idx, residual_dpcm);

        // IMPORTANT: In lossless mode, the "residual" is exactly what was in the bitstream
        if c_idx == 0 {
            // We still need to back up Luma for potential CCP in Chroma
            for i in 0..n_t * n_t {
                ctx.luma_residual_temp[i] = ctx.math_scratchpad[i];
            }
        }
        return; // Exit early; no transform/scaling needed
    }

    // --- 3. NORMAL PATH (Scaling & Transform) ---
    // We run scaling if there is a CBF OR if Chroma CCP is active (even if CBF is 0)
    let ccp_active = c_idx != 0 && ctx.res_scale_val != 0;

    let bit_depth = if c_idx == 0 { ctx.sps.bit_depth_luma } else { ctx.sps.bit_depth_chroma };
    if cbf || ccp_active {
        // Scale coefficients into math_scratchpad
        ctx.scale_coefficients(x0, y0, n_t, c_idx);

        // Inverse Transform
        if ctx.transform_skip_flag[c_idx] == 0 {
            // Pick DST for Luma 4x4 Intra, otherwise IDCT
            let use_dst = c_idx == 0 && n_t == 4 && cu_pred_mode == PredMode::ModeIntra;

            // scratchpad can be reused without reset-ing as the buffer is overwritten
            // so no need to reset it every time
            let idct_scratchpad: &mut [i16; 1024] = &mut ctx.idct_scratchpad[..].try_into().unwrap();

            if use_dst {
                // Luma 4x4 Intra -> Special DST path
                let block: &mut [i16; 16] = (&mut ctx.math_scratchpad[..16])
                    .try_into()
                    .expect("Scratchpad must have at least 16 elements");
                idst_4x4_hevc(block, idct_scratchpad, bit_depth);
            } else {
                // Standard IDCT Path
                match n_t {
                    4 => {
                        let block: &mut [i16; 16] =
                            (&mut ctx.math_scratchpad[..16]).try_into().unwrap();

                        idct_4x4_hevc(block, idct_scratchpad, bit_depth);
                    }
                    8 => {
                        let block: &mut [i16; 64] =
                            (&mut ctx.math_scratchpad[..64]).try_into().unwrap();

                        idct_8x8_hevc(block, idct_scratchpad, bit_depth);
                    }
                    16 => {
                        let block: &mut [i16; 256] =
                            (&mut ctx.math_scratchpad[..256]).try_into().unwrap();

                        idct_16x16_hevc(block, idct_scratchpad, bit_depth);
                    }
                    32 => {
                        let block: &mut [i16; 1024] = ctx
                            .math_scratchpad
                            .get_mut(..1024)
                            .unwrap()
                            .try_into()
                            .unwrap();
                        idct_32x32_hevc(block, idct_scratchpad, bit_depth);
                    }
                    _ => unreachable!("HEVC TU sizes are 4, 8, 16, or 32")
                }
            }
        } else {
            // RDPCM for Transform Skip (Spec 8.6.4.4.1)
            // --- 1. RDPCM for Transform Skip (Spec 8.6.4.4.1) ---
            match residual_dpcm {
                1 => ctx.apply_rdpcm_horizontal_in_place(n_t),
                2 => ctx.apply_rdpcm_vertical_in_place(n_t),
                _ => {}
            }

            // --- 2. The Missing Transform Skip Shift! (Spec 8.6.2.1) ---
            // Because we bypassed the IDCT, we must manually apply the
            // post-transform bit-shift to scale the residuals back down to pixel space.
            let shift = 20 - bit_depth as i32;
            let offset = if shift > 0 { 1 << (shift - 1) } else { 0 };

            for i in 0..(n_t * n_t) {
                let val = ctx.math_scratchpad[i] as i32;
                // Shift, and safely clamp to the 16-bit intermediate range
                let shifted = (val + offset) >> shift;
                ctx.math_scratchpad[i] = shifted.clamp(-32768, 32767) as i16;
            }
        }

        // Apply CCP for Chroma
        if c_idx != 0 && ctx.res_scale_val != 0 {
            let scale = ctx.res_scale_val;
            ctx.apply_cross_component_prediction(n_t, scale, None);
        }

        // --- 4. SAVE LUMA TEMPLATE ---
        // After Luma transform/scaling is done, but BEFORE we add to pixels,
        // save the residual result for Cb and Cr to use.
        if c_idx == 0 {
            ctx.luma_residual_temp[..n_t * n_t].copy_from_slice(&ctx.math_scratchpad[..n_t * n_t]);
        }

        // --- 5. RECONSTRUCT (Residuals + Prediction) ---
        ctx.add_residual_and_write(x0, y0, n_t, c_idx, None, bit_depth);
    } else {
        // No coefficients and no CCP: Just copy prediction pixels to frame
        ctx.write_block_scratchpad(c_idx, x0, y0, n_t, bit_depth);
    }
}

pub fn read_transform_tree(
    ctx: &mut DecodeSliceContext,
    x0: usize,
    y0: usize,
    x_base: usize,
    y_base: usize,
    log2_trafo_size: u8,
    trafo_depth: u8,
    max_trafo_depth: u8,
    blk_idx: usize, // New: identifies the quadrant (0-3)
    intra_split_flag: bool,
    mut cbf_cb: u8,
    mut cbf_cr: u8
) -> Result<(), NalError> {
    debug_more!(
        "read_transform_tree: x0:{} y0:{} log2_trafo_size:{} trafo_depth:{} blk_idx:{}",
        x0,
        y0,
        log2_trafo_size,
        trafo_depth,
        blk_idx
    );

    // 1. Determine split_flag
    let  split_flag;
    let can_decode_flag = log2_trafo_size <= ctx.sps.log2_max_transform_block_size
        && log2_trafo_size > ctx.sps.log2_min_transform_block_size
        && trafo_depth < max_trafo_depth
        && !(intra_split_flag && trafo_depth == 0);

    if can_decode_flag {
        split_flag = decode_split_transform_flag(ctx, log2_trafo_size);
    } else {
        // Inference logic (Size limits, Forced Intra NxN, or Inter hierarchy)
        let part_mode = ctx.neighbor_tracker.get_part_mode(x0, y0);
        let size_too_big = log2_trafo_size > ctx.sps.log2_max_transform_block_size;
        let forced_intra_split = intra_split_flag && trafo_depth == 0;
        let inter_split_flag = ctx.sps.max_transform_hierarchy_depth_inter == 0
            && trafo_depth == 0
            && !ctx.is_intra
            && part_mode != PartMode::Part2Nx2N;

        split_flag = size_too_big || forced_intra_split || inter_split_flag;
    }

    // 2. Decode Chroma CBFs
    // If the parent TU had a CBF of 0, all children are inferred to be 0.
    // If parent was 1, we decode a flag to see if this specific TU has coefficients.
    let has_chroma = (log2_trafo_size > 2 && ctx.sps.chroma_format != ChromaFormat::Monochrome)
        || ctx.sps.chroma_format == ChromaFormat::Yuv444;

    if has_chroma {
        // Cb Component
        if cbf_cb != 0 {
            let mut bit = decode_cbf_chroma(ctx, trafo_depth) as u8;
            if ctx.sps.chroma_format == ChromaFormat::Yuv422
                && (!split_flag || log2_trafo_size == 3)
            {
                bit |= (decode_cbf_chroma(ctx, trafo_depth) as u8) << 1;
            }
            cbf_cb = bit;
        }
        // Cr Component
        if cbf_cr != 0 {
            let mut bit = decode_cbf_chroma(ctx, trafo_depth) as u8;
            if ctx.sps.chroma_format == ChromaFormat::Yuv422
                && (!split_flag || log2_trafo_size == 3)
            {
                bit |= (decode_cbf_chroma(ctx, trafo_depth) as u8) << 1;
            }
            cbf_cr = bit;
        }
    }

    if split_flag {
        let half_size = 1 << (log2_trafo_size - 1);

        for j in 0..2 {
            for i in 0..2 {
                // Recursively call with the updated quadrant index
                read_transform_tree(
                    ctx,
                    x0 + i * half_size,
                    y0 + j * half_size,
                    x_base,
                    y_base,
                    log2_trafo_size - 1,
                    trafo_depth + 1,
                    max_trafo_depth,
                    j * 2 + i, // New blk_idx
                    intra_split_flag,
                    cbf_cb,
                    cbf_cr
                )?;
            }
        }
    } else {
        // 3. Leaf Node: Decode/Infer Luma CBF
        let cbf_luma = if ctx.is_intra || trafo_depth != 0 || cbf_cb != 0 || cbf_cr != 0 {
            decode_cbf_luma(ctx, trafo_depth)
        } else {
            true // Inferred: if everything else is 0, Luma MUST be 1 for a leaf
        };

        // 4. Transform Unit Processing
        // Passing the full set of parameters to handle 4:2:0 and 4:2:2 logic
        read_transform_unit(
            ctx,
            x0,
            y0,
            x_base,
            y_base,
            log2_trafo_size,
            blk_idx,
            cbf_luma,
            cbf_cb,
            cbf_cr
        )?;
    }
    Ok(())
}

fn decode_log2_res_scale_abs_plus1(ctx: &mut DecodeSliceContext, c_idx_minus_1: usize) -> u8 {
    debug_more!(" log2_res_scale_abs_plus1(c={})", c_idx_minus_1);

    let mut value = 0;
    let c_max = 4;

    for bin_idx in 0..c_max {
        // libde265 logic: 4 contexts per component
        let ctx_idx_inc = 4 * c_idx_minus_1 + bin_idx;

        let bit = ctx
            .cabac
            .decode_decision(CONTEXT_MODEL_LOG2_RES_SCALE_ABS_PLUS1 + ctx_idx_inc);

        if bit == 0 {
            break;
        }
        value += 1;
    }
    debug_more!(" decode_log2_res_scale_abs_plus1(value={})", value);

    value
}
fn decode_res_scale_sign_flag(ctx: &mut DecodeSliceContext, c_idx_minus_1: usize) -> u8 {
    // Context index 0 for Cb, 1 for Cr
    debug_more!(" decode_res_scale_sign_flag(c={})", c_idx_minus_1);
    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_RES_SCALE_SIGN_FLAG + c_idx_minus_1);
    debug_more!(" decode_res_scale_sign_flag(bit={})", bit);

    bit
}
pub fn read_cross_comp_pred(ctx: &mut DecodeSliceContext, c_idx_minus_1: usize) -> i8 {
    let log2_res_scale_abs_plus1 = decode_log2_res_scale_abs_plus1(ctx, c_idx_minus_1);
    let mut res_scale_val: i32;

    if log2_res_scale_abs_plus1 != 0 {
        let res_scale_sign_flag = decode_res_scale_sign_flag(ctx, c_idx_minus_1);

        // ResScaleVal = 2^(log2 - 1)
        res_scale_val = 1 << (log2_res_scale_abs_plus1 - 1);

        // Apply sign: 0 -> *1, 1 -> *-1
        res_scale_val *= 1 - 2 * (res_scale_sign_flag as i32);
    } else {
        res_scale_val = 0;
    }

    // Store in context for the upcoming TU reconstruction
    res_scale_val as i8
}
pub fn read_transform_unit(
    ctx: &mut DecodeSliceContext,
    x0: usize,
    y0: usize,
    x_base: usize, // Base of the CU/TU group
    y_base: usize,
    log2_size: u8,
    blk_idx: usize,
    cbf_luma: bool,
    cbf_cb: u8, // 2 bits for 4:2:2
    cbf_cr: u8
) -> Result<(), NalError> {
    let nt = 1 << log2_size;
    let chroma_format = ctx.sps.chroma_format;

    // 1. QP Delta Handling
    if (cbf_luma || cbf_cb != 0 || cbf_cr != 0) && ctx.pps.cu_qp_delta_enabled_flag {
        if !ctx.is_cu_qp_delta_coded {
            ctx.cu_qp_delta = decode_cu_qp_delta(ctx)?;
            ctx.is_cu_qp_delta_coded = true;
            decode_quantization_parameters(ctx, x0, y0, log2_size);
        }
    }

    // 2. Luma Path
    let pred_mode = ctx.neighbor_tracker.get_pred_mode(x0, y0);
    if cbf_luma {
        decode_residual_block(ctx, x0, y0, log2_size, Component::Luma);
    }
    // Scale -> Transform -> Reconstruct Luma
    decode_tu(ctx, x0, y0, nt, 0, pred_mode, cbf_luma);

    // 3. Chroma Path
    if chroma_format == ChromaFormat::Monochrome {
        return Ok(());
    }
    // In 4:2:0, if Luma is 4x4, we only process Chroma at the 4th block (blk_idx 3)
    let is_420_small = (chroma_format == ChromaFormat::Yuv420) && (log2_size == 2);
    let should_decode_chroma = !is_420_small || blk_idx == 3;

    if should_decode_chroma {
        // Calculate Chroma TU size
        // For 4:4:4 or 4:2:0 group-of-four, the chroma TU is the same size as luma (4x4)
        // For 8x8, 16x16, 32x32, chroma is half the luma size (except 4:4:4)
        let log2_size_c = if chroma_format == ChromaFormat::Yuv444 || is_420_small {
            log2_size
        } else {
            log2_size - 1
        };
        let nt_c = 1 << log2_size_c;

        // --- Cross Component Prediction (CCP) Check ---
        let cross_component_prediction_enabled_flag = ctx
            .pps
            .range_extension
            .as_ref()
            .map_or(false, |range| range.cross_component_prediction_enabled_flag);

        let do_ccp = cross_component_prediction_enabled_flag
            && cbf_luma
            && (pred_mode == PredMode::ModeInter || ctx.is_intra_pred_mode_c_mode4(x0, y0));

        // Subsampling factors
        let (sub_w, sub_h) = match chroma_format {
            ChromaFormat::Yuv420 => (2, 2),
            ChromaFormat::Yuv422 => (2, 1),
            ChromaFormat::Yuv444 => (1, 1),
            _ => (1, 1)
        };

        // --- Process Cb & Cr ---
        for c_idx in 1..=2 {
            let cbf = if c_idx == 1 { cbf_cb } else { cbf_cr };

            if do_ccp {
                ctx.res_scale_val = read_cross_comp_pred(ctx, c_idx - 1);
            } else {
                ctx.res_scale_val = 0;
            }

            // Top (or only) Chroma Block
            if (cbf & 1) != 0 {
                decode_residual_block(
                    ctx,
                    x_base,
                    y_base,
                    log2_size_c,
                    if c_idx == 1 { Component::Cb } else { Component::Cr },
                    
                );
            }
            decode_tu(
                ctx,
                x_base / sub_w,
                y_base / sub_h,
                nt_c,
                c_idx,
                pred_mode,
                (cbf & 1) != 0
            );

            // 4:2:2 Vertical Extension (Second Chroma Block)
            if chroma_format == ChromaFormat::Yuv422 {
                let y_offset = 1 << log2_size_c;
                if (cbf & 2) != 0 {
                    // Note: y_base + y_offset translated back to Luma coordinates
                    decode_residual_block(
                        ctx,
                        x_base,
                        y_base + (y_offset * sub_h),
                        log2_size_c,
                        if c_idx == 1 { Component::Cb } else { Component::Cr },
                        
                    );
                }
                decode_tu(
                    ctx,
                    x_base / sub_w,
                    y_base / sub_h + y_offset,
                    nt_c,
                    c_idx,
                    pred_mode,
                    (cbf & 2) != 0
                );
            }
        }
    }

    Ok(())
}
