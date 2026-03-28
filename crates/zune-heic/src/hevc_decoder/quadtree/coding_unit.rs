use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_CU_SKIP_FLAG, CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG,
    CONTEXT_MODEL_PRED_MODE_FLAG, CONTEXT_MODEL_RQT_ROOT_CBF
};
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, SliceType};
use crate::hevc_decoder::quadtree::intra::{decode_intra_chroma_mode, decode_intra_luma_mode};
use crate::hevc_decoder::quadtree::part_mode::decode_part_mode;
use crate::hevc_decoder::quadtree::sao::read_sao;
use crate::hevc_decoder::quadtree::{
    DecodeSliceContext, read_coding_quadtree
};
use crate::hevc_decoder::quadtree::quant::decode_quantization_parameters;

pub fn read_coding_unit(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log_2_cb_size: u8, ct_depth: u8
) {
    let cb_size = 1 << log_2_cb_size;
    let pps = ctx.pps;
    let shdr = ctx.slice_header;

    debug_more!("read_coding_unit x0={}, y0={}, size={}", x0, y0, cb_size);

    // 1. Derivation of Quantization Parameters
    decode_quantization_parameters(ctx, x0, y0, log_2_cb_size);

    // 2. cu_transquant_bypass_flag
    if pps.transquant_bypass_enabled_flag {
        let bypass_bit = ctx
            .cabac
            .decode_decision(CONTEXT_MODEL_CU_TRANSQUANT_BYPASS_FLAG);
        ctx.cu_transquant_bypass_flag = bypass_bit == 1;
    }

    // 3. cu_skip_flag
    ctx.is_skip = false;
    if shdr.slice_type != SliceType::I {
        let skip_ctx_inc = ctx.neighbor_tracker.derive_skip_context(x0, y0);
        let skip_bit = ctx
            .cabac
            .decode_decision(CONTEXT_MODEL_CU_SKIP_FLAG + skip_ctx_inc);
        ctx.is_skip = skip_bit == 1;
    }

    if ctx.is_skip {
        ctx.is_intra = false;
        debug_more!("CU Mode: SKIP");
        // read_prediction_unit_skip(ctx, x0, y0, cb_size);
        // Skip blocks have no residuals, so we exit early here.
        return;
    }

    // 4. pred_mode_flag
    if shdr.slice_type != SliceType::I {
        let mode_bit = ctx
            .cabac
            .decode_decision(CONTEXT_MODEL_PRED_MODE_FLAG);
        ctx.is_intra = mode_bit == 1;
    } else {
        ctx.is_intra = true;
    }

    // 5. Partition Mode (PartMode)
    // Decides if we use one 2Nx2N block or split into NxN, 2NxN, etc.
    let part_mode = decode_part_mode(ctx, log_2_cb_size);
    let intra_split_flag = (ctx.is_intra && part_mode == PartMode::PartNxN) as u8;

    if ctx.is_intra {
        // 6. INTRA: Decode Luma and Chroma modes
        if part_mode == PartMode::PartNxN {
            let pb_size = cb_size / 2; // e.g., 4 if cb_size is 8
            for j in 0..2 {
                for i in 0..2 {
                    let curr_x = x0 + i * pb_size;
                    let curr_y = y0 + j * pb_size;

                    let mode = decode_intra_luma_mode(ctx, curr_x, curr_y);

                    // Update the tracker immediately so the NEXT sub-block can see this mode
                    ctx.neighbor_tracker
                        .set_intra_mode(curr_x, curr_y, pb_size, mode);
                }
            }
        } else {
            let mode = decode_intra_luma_mode(ctx, x0, y0);
            ctx.neighbor_tracker.set_intra_mode(x0, y0, cb_size, mode);
            ctx.intra_mode_luma = mode;
        }

        if ctx.sps.chroma_format != ChromaFormat::Monochrome {
            ctx.intra_mode_chroma = decode_intra_chroma_mode(ctx, ctx.intra_mode_luma);
        }
    } else {
        // 7. INTER: Motion Vectors (Placeholder)
        // read_prediction_unit(ctx, x0, y0, ...);
    }

    // 8. Residuals (Transform Tree)
    let mut rqt_root_cbf = true;
    if !ctx.is_intra {
        // For Inter, check if there are any residuals at all
        rqt_root_cbf = ctx.cabac.decode_decision(CONTEXT_MODEL_RQT_ROOT_CBF) == 1;
    }

    if rqt_root_cbf {
        let max_trafo_depth = if ctx.is_intra {
            ctx.sps.max_transform_hierarchy_depth_intra + (intra_split_flag as u64)
        } else {
            ctx.sps.max_transform_hierarchy_depth_inter
        };

        // This starts the recursive DCT/DST coefficient decoding
        todo!()
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
