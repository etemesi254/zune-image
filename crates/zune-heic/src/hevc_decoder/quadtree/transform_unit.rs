use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac::CabacDecoder;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_CBF_CHROMA, CONTEXT_MODEL_CBF_LUMA, CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG,
    CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG, CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG,
    CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX,
    CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX, CONTEXT_MODEL_SPLIT_TRANSFORM_FLAG
};
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::nal_parser::NalError;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::neighbor_tracker::PredMode;
use crate::hevc_decoder::quadtree::DecodeSliceContext;
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
    assert!(context >= 0 && context <= 2);
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
    if cu_pred_mode == PredMode::ModeIntra {
        // --- 1. Get Intra Prediction Mode ---
        let intra_pred_mode = if c_idx == 0 {
            ctx.neighbor_tracker.get_intra_mode(x0, y0)
        } else {
            let sub_width_c = sps.sub_width_c as usize;
            let sub_height_c = sps.sub_height_c as usize;
            ctx.neighbor_tracker
                .get_intra_mode_chroma(x0 * sub_width_c, y0 * sub_width_c + sub_width_c)
        };
        if  intra_pred_mode >= 35 {
            panic!("intra_pred_mode cannot be more than 35");
        }

        // --- 2. Perform Intra Prediction ---
        // This fills the prediction buffer with spatial directions
        decode_intra_prediction(ctx, x0, y0, intra_pred_mode, n_t, c_idx);
        panic!();
       
    } else {
       
    }
}

pub fn read_transform_tree(
    ctx: &mut DecodeSliceContext,
    x0: usize,
    y0: usize, // Current block top-left
    x_base: usize,
    y_base: usize, // CU top-left
    log2_trafo_size: u8,
    trafo_depth: u8,
    max_trafo_depth: u8,
    intra_split_flag: bool,
    mut cbf_cb: bool,
    mut cbf_cr: bool
) -> Result<(), NalError> {
    debug_more!(
        " ---- read_transform_tree(interleaved) x0:{} y0:{} x_base:{},y_base:{},log2_trafo_size:{},trafo_depth:{},max_trafo_depth:{}",
        x0,
        y0,
        x_base,
        y_base,
        log2_trafo_size,
        trafo_depth,
        max_trafo_depth
    );

    // 1. Determine if we decode or infer the split flag
    let cbf_luma;
    let mut split_flag;

    // Logic for "Can we even choose?"
    // We can ONLY choose if:
    // - We are within the min/max size bounds
    // - We haven't reached the max depth
    // - AND it's NOT a forced Intra split (Intra NxN at depth 0)
    let can_decode_flag = log2_trafo_size <= ctx.sps.log2_max_transform_block_size
        && log2_trafo_size > ctx.sps.log2_min_transform_block_size
        && trafo_depth < max_trafo_depth
        && !(intra_split_flag && trafo_depth == 0);

    if can_decode_flag {
        // Decode from bitstream
        // Note: HEVC uses the size to pick the context
        split_flag = decode_split_transform_flag(ctx, log2_trafo_size);
    } else {
        // INFERENCE LOGIC
        let part_mode = ctx.neighbor_tracker.get_part_mode(x0, y0);

        // Case A: Size too big
        let size_too_big = log2_trafo_size > ctx.sps.log2_max_transform_block_size;

        // Case B: Forced Intra NxN split at the root
        let forced_intra_split = intra_split_flag && trafo_depth == 0;

        // Case C: Inter split edge case
        // (Matches libde265: depth 0, hierarchy 0, non-2Nx2N Inter)
        let inter_split_flag = ctx.sps.max_transform_hierarchy_depth_inter == 0
            && trafo_depth == 0
            && !ctx.is_intra
            && part_mode != PartMode::Part2Nx2N;

        split_flag = size_too_big || forced_intra_split || inter_split_flag;
    }

    // 2. Decode Chroma CBFs (Coded Block Flags)
    // Only decode if we have chroma data
    let has_chroma = (log2_trafo_size > 2 && ctx.sps.chroma_format != ChromaFormat::Monochrome)
        || ctx.sps.chroma_format == ChromaFormat::Yuv444;

    if has_chroma {
        // 1. Process Cb (Chroma Blue)
        if cbf_cb {
            let mut bit = decode_cbf_chroma(ctx, trafo_depth);

            // 4:2:2 Special Case: Read second CBF bit if necessary
            if ctx.sps.chroma_format == ChromaFormat::Yuv422
                && (!split_flag || log2_trafo_size == 3)
            {
                let second_bit = decode_cbf_chroma(ctx, trafo_depth);
                bit |= second_bit << 1;
            }
            cbf_cb = bit != 0;
        }

        // 2. Process Cr (Chroma Red)
        if cbf_cr {
            let mut bit = decode_cbf_chroma(ctx, trafo_depth);

            // 4:2:2 Special Case: Read second CBF bit if necessary
            if ctx.sps.chroma_format == ChromaFormat::Yuv422
                && (!split_flag || log2_trafo_size == 3)
            {
                let second_bit = decode_cbf_chroma(ctx, trafo_depth);
                bit |= second_bit << 1;
            }
            cbf_cr = bit != 0;
        }
    }

    if split_flag {
        let sub_size = 1 << (log2_trafo_size - 1);
        let half_size = sub_size;

        debug_more!("transform_split (sub_size: {sub_size}, half_size: {half_size})");

        // Recursive split into 4 quadrants
        for j in 0..2 {
            for i in 0..2 {
                read_transform_tree(
                    ctx,
                    x0 + i * half_size,
                    y0 + j * half_size,
                    x_base,
                    y_base,
                    log2_trafo_size - 1,
                    trafo_depth + 1,
                    max_trafo_depth,
                    intra_split_flag,
                    cbf_cb,
                    cbf_cr
                )?;
            }
        }

        Ok(())
    } else {
        // 3. Leaf Node: Decode Luma CBF
        // Intra blocks at depth 0 ALWAYS have cbf_luma = 1 if not explicitly split.
        if ctx.is_intra || trafo_depth != 0 || cbf_cb || cbf_cr {
            cbf_luma = decode_cbf_luma(ctx, trafo_depth);
        } else {
            cbf_luma = true; // Inferred
        }

        // 4. Enter the Transform Unit (Residual Coding)
        read_transform_unit(ctx, x0, y0, log2_trafo_size, cbf_luma, cbf_cb, cbf_cr)?;
        Ok(())
    }
}

fn read_transform_unit(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log2_size: u8, cbf_luma: bool,
    cbf_cb: bool, cbf_cr: bool
) -> Result<(), NalError> {
    debug_more!(
        "---read_transform_unit(x0={},y0={},log2size={})",
        x0,
        y0,
        log2_size
    );

    let nt = 1 << log2_size;

    let pred_mode = ctx.neighbor_tracker.get_pred_mode(x0, y0);
    // 1. HEVC Spec §7.3.8.11: cu_qp_delta is decoded here if enabled
    // and not yet coded for the current Quantization Group (QG).
    if (cbf_luma || cbf_cb || cbf_cr) && ctx.pps.cu_qp_delta_enabled_flag {
        if !ctx.is_cu_qp_delta_coded {
            ctx.cu_qp_delta = decode_cu_qp_delta(ctx)?;
            ctx.is_cu_qp_delta_coded = true;

            // Recalculate QPs now that we have the delta
            decode_quantization_parameters(ctx, x0, y0, log2_size);
        }
    }
    // 1. Deciding on DST (Discrete Sine Transform)
    // HEVC Spec §8.4.4.1: DST is used ONLY for Luma 4x4 Intra blocks.
    let use_dst = ctx.is_intra && log2_size == 2;

    if cbf_luma {
        decode_residual_block(ctx, x0, y0, log2_size, Component::Luma, use_dst);
    }
    decode_tu(ctx, x0, y0, nt, 0, pred_mode, cbf_luma);
    // Chroma blocks ALWAYS use DCT-II
    if cbf_cb {
        decode_residual_block(ctx, x0, y0, log2_size - 1, Component::Cb, false);
    }
    if cbf_cr {
        decode_residual_block(ctx, x0, y0, log2_size - 1, Component::Cr, false);
    }
    Ok(())
}
