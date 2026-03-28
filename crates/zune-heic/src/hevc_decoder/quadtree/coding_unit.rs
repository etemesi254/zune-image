use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_CU_SKIP_FLAG, CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG,
    CONTEXT_MODEL_PRED_MODE_FLAG, CONTEXT_MODEL_RQT_ROOT_CBF
};
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::constants::PartMode::Part2Nx2N;
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, SliceType};
use crate::hevc_decoder::neighbor_tracker::PredMode;
use crate::hevc_decoder::quadtree::intra::{
    decode_intra_chroma_mode, decode_intra_luma_mode, decode_prev_intra_luma_pred_flag
};
use crate::hevc_decoder::quadtree::part_mode::decode_part_mode;
use crate::hevc_decoder::quadtree::quant::decode_quantization_parameters;
use crate::hevc_decoder::quadtree::sao::read_sao;
use crate::hevc_decoder::quadtree::transform_unit::read_transform_tree;
use crate::hevc_decoder::quadtree::{DecodeSliceContext, read_coding_quadtree};

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
pub fn read_coding_unit(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log_2_cb_size: u8, ct_depth: u8
) {
    let cb_size = 1 << log_2_cb_size;
    let shdr = ctx.slice_header;

    debug_more!("read_coding_unit x0={}, y0={}, size={}", x0, y0, cb_size);

    decode_quantization_parameters(ctx, x0, y0, log_2_cb_size);
    // 1. cu_transquant_bypass_flag
    if ctx.pps.transquant_bypass_enabled_flag {
        ctx.cu_transquant_bypass_flag = decode_cu_transquant_bypass_flag(ctx);
    }

    // 2. cu_skip_flag
    ctx.is_skip = false;
    if shdr.slice_type != SliceType::I {
        ctx.is_skip = decode_cu_skip_flag(ctx, x0, y0);
    }

    if ctx.is_skip {
        ctx.is_intra = false;
        // Inter skip MV logic would go here
        ctx.neighbor_tracker
            .set_pred_mode(x0, y0, log_2_cb_size, PredMode::ModeSkip);
        todo!();
        return;
    }

    // 3. pred_mode_flag (Only for P/B slices)
    ctx.is_intra = if shdr.slice_type != SliceType::I { decode_pred_mode_flag(ctx) } else { true };

    let mode = if ctx.is_intra { PredMode::ModeIntra } else { PredMode::ModeInter };
    ctx.neighbor_tracker
        .set_pred_mode(x0, y0, log_2_cb_size, mode);

    // 4. Partition Mode Inference (CRITICAL FIX)
    let part_mode = if !ctx.is_intra || log_2_cb_size == ctx.sps.log2_min_luma_coding_block_size {
        decode_part_mode(ctx, log_2_cb_size)
    } else {
        // If it's Intra and NOT the min size, it's ALWAYS 2Nx2N
        PartMode::Part2Nx2N
    };
    // store it
    ctx.neighbor_tracker
        .set_part_mode(x0, y0, log_2_cb_size, part_mode);

    let intra_split_flag = ctx.is_intra && part_mode == PartMode::PartNxN;

    // 5. PCM
    let mut pcm_flag = false;
    if ctx.is_intra {
        if ctx.is_intra && part_mode == PartMode::Part2Nx2N && ctx.sps.pcm_enabled_flag {
            // Check if the current size is within the allowed PCM range
            let log_2_max_ipcm_size_y = ctx.sps.log2_min_pcm_luma_coding_block_size
                + ctx.sps.log2_diff_max_min_pcm_luma_coding_block_size;

            if log_2_cb_size >= ctx.sps.log2_min_pcm_luma_coding_block_size
                && log_2_cb_size <= log_2_max_ipcm_size_y
            {
                pcm_flag = ctx.cabac.decode_terminate() == 1;
            }

            if pcm_flag {
                debug_more!("CU Mode: PCM at [{}, {}]", x0, y0);
                // read_pcm_samples(ctx, x0, y0, log_2_cb_size);
                todo!();
                return; // PCM blocks have no standard intra modes and NO residuals.
            }
        }
        // 5. Intra Mode Decoding

        if part_mode == PartMode::PartNxN {
            let pb_size = cb_size / 2;
            let mut prev_mpm_flags = [false; 4];

            // --- PASS 1: Decode all 4 flags first ---
            let mut idx = 0;
            for j in 0..2 {
                for i in 0..2 {
                    // This is the call that happens 4 times in the first loop
                    prev_mpm_flags[idx] = decode_prev_intra_luma_pred_flag(ctx) == 1;
                    idx += 1;
                }
            }

            // --- PASS 2: Decode the 4 modes/indices using those flags ---
            idx = 0;
            for j in 0..2 {
                for i in 0..2 {
                    let curr_x = x0 + i * pb_size;
                    let curr_y = y0 + j * pb_size;

                    // Pass the pre-decoded flag into a specialized function
                    let mode = decode_intra_luma_mode(ctx, curr_x, curr_y, prev_mpm_flags[idx]);

                    ctx.neighbor_tracker
                        .set_intra_mode(curr_x, curr_y, pb_size, mode);

                    // Chroma DM mode always looks at the top-left (PU0)
                    if i == 0 && j == 0 {
                        ctx.intra_mode_luma = mode;
                    }
                    idx += 1;
                }
            }
        } else {
            // 2Nx2N: Single pass is safe here
            let is_mpm = decode_prev_intra_luma_pred_flag(ctx) == 1;
            let mode = decode_intra_luma_mode(ctx, x0, y0, is_mpm);
            ctx.neighbor_tracker.set_intra_mode(x0, y0, cb_size, mode);
            ctx.intra_mode_luma = mode;
        }

        if ctx.sps.chroma_format != ChromaFormat::Monochrome {
            ctx.intra_mode_chroma = decode_intra_chroma_mode(ctx, ctx.intra_mode_luma);
        }
    } else {
        // 8. INTER PROCESSING (Prediction Units)
        let n_cs = 1 << log_2_cb_size;

        todo!()
        //     match part_mode {
        //         PartMode::Part2Nx2N => {
        //             read_prediction_unit(ctx, x0, y0, n_cs, n_cs, 0);
        //         }
        //         PartMode::Part2NxN => {
        //             read_prediction_unit(ctx, x0, y0, n_cs, n_cs / 2, 0);
        //             read_prediction_unit(ctx, x0, y0 + (n_cs / 2), n_cs, n_cs / 2, 1);
        //         }
        //         PartMode::PartNx2N => {
        //             read_prediction_unit(ctx, x0, y0, n_cs / 2, n_cs, 0);
        //             read_prediction_unit(ctx, x0 + (n_cs / 2), y0, n_cs / 2, n_cs, 1);
        //         }
        //         PartMode::PartNxN => {
        //             let pb_size = n_cs / 2;
        //             for j in 0..2 {
        //                 for i in 0..2 {
        //                     read_prediction_unit(ctx, x0 + i * pb_size, y0 + j * pb_size, pb_size, pb_size, (j * 2 + i) as u8);
        //                 }
        //             }
        //         }
        //         // Asymmetric Motion Partitions (AMP)
        //         PartMode::Part2NxnU | PartMode::Part2NxnD | PartMode::PartnLx2N | PartMode::PartnRx2N => {
        //             decode_amp_prediction_units(ctx, x0, y0, n_cs, part_mode);
        //         }
        // }
    }
    // 9. RESIDUAL PROCESSING (Transform Tree)
    if pcm_flag == false {
        let mut rqt_root_cbf = true;
        if !ctx.is_intra {
            rqt_root_cbf = decode_rqt_root_cbf(ctx);
        }

        if rqt_root_cbf {
            let max_trafo_depth = if ctx.is_intra {
                ctx.sps.max_transform_hierarchy_depth_intra + (intra_split_flag as u64)
            } else {
                ctx.sps.max_transform_hierarchy_depth_inter
            };

            // It happens inside the transform tree when the first CBF=1 is found.
            read_transform_tree(
                ctx,
                x0,
                y0,
                x0,
                y0,
                log_2_cb_size,
                0,
                max_trafo_depth as u8,
                intra_split_flag,
                true,
                true
            );
        }
    }
}

pub fn read_coding_tree_unit(ctx: &mut DecodeSliceContext, ctu_x: usize, ctu_y: usize) {
    let sps = ctx.sps;
    let pps = ctx.pps;
    let shdr = &ctx.slice_header;

    let log_2_ctb_size_y = sps.log2_ctb_size_y;
    let x_ctb_pixels = ctu_x << log_2_ctb_size_y;
    let y_ctb_pixels = ctu_y << log_2_ctb_size_y;

    debug_more!(
        "DECODE CTB log_2_ctb_size_y: {log_2_ctb_size_y} x_ctb_pixels: {x_ctb_pixels} y_ctb_pixels: {y_ctb_pixels}"
    );
    if shdr.slice_sao_luma_flag || shdr.slice_sao_chroma_flag {
        let sao_info = read_sao(ctx, x_ctb_pixels, y_ctb_pixels);
    }

    read_coding_quadtree(ctx, x_ctb_pixels, y_ctb_pixels, log_2_ctb_size_y, 0);

    let z = 0;
}
