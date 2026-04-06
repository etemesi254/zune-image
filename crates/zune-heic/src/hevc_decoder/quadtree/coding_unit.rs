use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_CU_SKIP_FLAG, CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG,
    CONTEXT_MODEL_PRED_MODE_FLAG, CONTEXT_MODEL_RQT_ROOT_CBF
};
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_parser::NalError;
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, SliceType};
use crate::hevc_decoder::neighbor_tracker::PredMode;
use crate::hevc_decoder::quadtree::intra::{
    decode_intra_chroma_mode, decode_intra_luma_mode, decode_prev_intra_luma_pred_flag
};
use crate::hevc_decoder::quadtree::part_mode::decode_part_mode;
use crate::hevc_decoder::quadtree::quant::decode_quantization_parameters;
use crate::hevc_decoder::quadtree::read_coding_quadtree;
use crate::hevc_decoder::quadtree::sao::read_sao;
use crate::hevc_decoder::quadtree::transform_unit::read_transform_tree;


fn decode_rqt_root_cbf(ctx: &mut DecodeSliceContext) -> bool {
    debug_more!("decode_rqt_root_cbf");
    let rqt_root_cbf = ctx.cabac.decode_decision(CONTEXT_MODEL_RQT_ROOT_CBF) == 1;
    debug_more!("  decode_rqt_root_cbf={}", rqt_root_cbf);
    rqt_root_cbf
}

fn decode_pred_mode_flag(ctx: &mut DecodeSliceContext) -> bool {
    debug_more!("decode_pred_mode_flag");
    let bit = ctx.cabac.decode_decision(CONTEXT_MODEL_PRED_MODE_FLAG) == 1;
    debug_more!("  decode_pred_mode_flag={}", bit);
    bit
}
fn decode_cu_skip_flag(ctx: &mut DecodeSliceContext, x0: usize, y0: usize) -> bool {
    debug_more!("decode_cu_skip_flag");
    let skip_ctx = ctx.neighbor_tracker.derive_skip_context(x0, y0);
    let is_skip = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_CU_SKIP_FLAG + skip_ctx)
        == 1;
    debug_more!("  decode_cu_skip_flag={}", is_skip);
    return is_skip;
}
fn decode_cu_transquant_bypass_flag(ctx: &mut DecodeSliceContext) -> bool {
    debug_more!("decode_cu_transquant_bypass_flag");
    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG)
        == 1;
    debug_more!("  decode_cu_transquant_bypass_flag={}", bit);
    bit
}

pub fn get_actual_chroma_mode(intra_mode_chroma: u8, intra_mode_luma: u8) -> u8 {
    // intra_mode_chroma is the 0-4 value from the bitstream
    // intra_mode_luma is the 0-34 value from the corresponding Luma block

    let mut actual_mode = match intra_mode_chroma {
        0 => 0,                      // Planar
        1 => 26,                     // Vertical
        2 => 10,                     // Horizontal
        3 => 1,                      // DC
        4 => return intra_mode_luma, // DM: Just return Luma's mode
        _ => 1                       // Safety fallback to DC
    };

    // If the mapped mode is the SAME as the Luma mode,
    // HEVC "swaps" it to Mode 34 (Intra_Angular 34)
    if actual_mode == intra_mode_luma {
        actual_mode = 34;
    }

    actual_mode
}
pub fn read_coding_unit(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log_2_cb_size: u8,
) -> Result<(), NalError> {
    let cb_size = 1 << log_2_cb_size;
    let shdr = ctx.slice_header;

    debug_more!("read_coding_unit x0={}, y0={}, size={}", x0, y0, cb_size);

    // 1. Initial Quantization Setup
    // Note: Actual QP delta decoding is moved into TU for some profiles,
    // but we'll keep your call here for base parameters.
    decode_quantization_parameters(ctx, x0, y0, log_2_cb_size);

    if ctx.pps.transquant_bypass_enabled_flag {
        ctx.cu_transquant_bypass_flag = decode_cu_transquant_bypass_flag(ctx);
    }

    // 2. Skip Flag
    ctx.is_skip = false;
    if shdr.slice_type != SliceType::I {
        ctx.is_skip = decode_cu_skip_flag(ctx, x0, y0);
    }

    if ctx.is_skip {
        ctx.is_intra = false;
        ctx.neighbor_tracker
            .set_pred_mode(x0, y0, log_2_cb_size, PredMode::ModeSkip);
        // Inter skip MV logic (Merge/Skip) would be handled here
        return Ok(());
    }

    // 3. Prediction Mode
    ctx.is_intra = if shdr.slice_type == SliceType::I { true } else { decode_pred_mode_flag(ctx) };
    let mode = if ctx.is_intra { PredMode::ModeIntra } else { PredMode::ModeInter };
    ctx.neighbor_tracker
        .set_pred_mode(x0, y0, log_2_cb_size, mode);

    // 4. Partition Mode
    let part_mode = if !ctx.is_intra || log_2_cb_size == ctx.sps.log2_min_luma_coding_block_size {
        decode_part_mode(ctx, log_2_cb_size)
    } else {
        PartMode::Part2Nx2N
    };
    ctx.neighbor_tracker
        .set_part_mode(x0, y0, log_2_cb_size, part_mode);

    let intra_split_flag = ctx.is_intra && part_mode == PartMode::PartNxN;

    // 5. Intra Prediction Decoding
    let mut pcm_flag = false;
    if ctx.is_intra {
        // PCM Logic
        if part_mode == PartMode::Part2Nx2N && ctx.sps.pcm_enabled_flag {
            let log_2_min_pcm = ctx.sps.log2_min_pcm_luma_coding_block_size;
            let log_2_max_pcm =
                log_2_min_pcm + ctx.sps.log2_diff_max_min_pcm_luma_coding_block_size;

            if log_2_cb_size >= log_2_min_pcm && log_2_cb_size <= log_2_max_pcm {
                pcm_flag = ctx.cabac.decode_terminate() == 1;
            }
        }

        if pcm_flag {
            // read_pcm_samples(...)
            todo!();
        } else {
            // Decode Luma Intra Modes
            debug_more!("Part Mode:{:?}",part_mode);
            if part_mode == PartMode::PartNxN {
                let pb_size = cb_size / 2;
                let mut prev_mpm_flags = [false; 4];
                for i in 0..4 {
                    prev_mpm_flags[i] = decode_prev_intra_luma_pred_flag(ctx) == 1;
                }

                for idx in 0..4 {
                    let curr_x = x0 + (idx % 2) * pb_size;
                    let curr_y = y0 + (idx / 2) * pb_size;
                    let mode = decode_intra_luma_mode(ctx, curr_x, curr_y, prev_mpm_flags[idx]);
                    ctx.neighbor_tracker
                        .set_intra_mode(curr_x, curr_y, pb_size, mode);
                    if idx == 0 {
                        ctx.intra_mode_luma = mode;
                    }
                }
            } else {
                let is_mpm = decode_prev_intra_luma_pred_flag(ctx) == 1;
                let mode = decode_intra_luma_mode(ctx, x0, y0, is_mpm);
                ctx.neighbor_tracker.set_intra_mode(x0, y0, cb_size, mode);
                ctx.intra_mode_luma = mode;
            }

            // Decode Chroma Intra Mode
            if ctx.sps.chroma_format != ChromaFormat::Monochrome {
                let idx = decode_intra_chroma_mode(ctx, ctx.intra_mode_luma);
                let actual_mode = get_actual_chroma_mode(idx, ctx.intra_mode_luma);
                debug_more!("Final actual chroma mode: {:?}", actual_mode);
                ctx.neighbor_tracker.set_intra_mode_chroma(
                    x0,
                    y0,
                    log_2_cb_size,
                    actual_mode,
                    idx == 4
                );
                ctx.intra_mode_chroma = actual_mode;
            }
        }
    } else {
        // Inter Prediction Units (MVs, Merges, etc.)
        todo!();
    }

    // 6. Residual Processing (Transform Tree)
    if !pcm_flag {
        let mut rqt_root_cbf = true;
        if !ctx.is_intra {
            rqt_root_cbf = decode_rqt_root_cbf(ctx);
        }

        if rqt_root_cbf {
            let max_trafo_depth = if ctx.is_intra {
                ctx.sps.max_transform_hierarchy_depth_intra + u64::from(intra_split_flag)
            } else {
                ctx.sps.max_transform_hierarchy_depth_inter
            };

            // START TRANSFORM TREE
            // x_base/y_base are set to the current CU origin (x0, y0)
            // blk_idx starts at 0. cbf_cb/cr start as u8 (value 1)
            read_transform_tree(
                ctx,
                x0,
                y0,
                x0, // x_base
                y0, // y_base
                log_2_cb_size,
                0, // trafo_depth
                max_trafo_depth as u8,
                0, // blk_idx
                intra_split_flag,
                1, // cbf_cb (u8)
                1  // cbf_cr (u8)
            )?;
        }
    }

    Ok(())
}

pub fn read_coding_tree_unit(
    ctx: &mut DecodeSliceContext, ctu_x: usize, ctu_y: usize
) -> Result<(), NalError> {
    let sps = ctx.sps;
    let shdr = &ctx.slice_header;

    let log_2_ctb_size_y = sps.log2_ctb_size_y;

    let x_ctb_pixels = ctu_x << log_2_ctb_size_y;
    let y_ctb_pixels = ctu_y << log_2_ctb_size_y;

    debug_more!(
        "DECODE CTB log_2_ctb_size_y: {log_2_ctb_size_y} x_ctb_pixels: {x_ctb_pixels} y_ctb_pixels: {y_ctb_pixels}"
    );
    if shdr.slice_sao_luma_flag || shdr.slice_sao_chroma_flag {
         read_sao(ctx, ctu_x, ctu_y);
    }

    read_coding_quadtree(ctx, x_ctb_pixels, y_ctb_pixels, log_2_ctb_size_y, 0)?;

    Ok(())
}
