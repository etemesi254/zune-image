use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_CU_QP_DELTA_ABS;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_parser::NalError;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;

pub fn decode_cu_qp_delta(ctx: &mut DecodeSliceContext) -> Result<i32, NalError> {
    debug_more!("# cu_qp_delta_abs");

    let ctx_base = CONTEXT_MODEL_CU_QP_DELTA_ABS;
    let mut abs_qp_delta: u32 = 0;

    // 1. Decode the first bin (Context 0)
    let first_bin = ctx.cabac.decode_decision(ctx_base);

    if first_bin == 1 {
        // We have at least a value of 1
        abs_qp_delta = 1;

        // 2. Decode up to 4 more prefix bins (Context 1)
        for _ in 0..4 {
            let bin = ctx.cabac.decode_decision(ctx_base + 1);
            if bin == 0 {
                break;
            }
            abs_qp_delta += 1;
        }

        // 3. Decode the Suffix if prefix reached 5
        if abs_qp_delta == 5 {
            let suffix_val = ctx.cabac.decode_bypass_eg0();
            abs_qp_delta += suffix_val;
            if suffix_val >= 250 {
                return Err(NalError::Generic(format!(
                    "CABAC Max bin >=250 {suffix_val}"
                )));
            }
        }
    }

    // Match libde265 trace: "$1 cu_qp_delta_abs=%d"
    debug_more!("  cu_qp_delta_abs={}", abs_qp_delta);

    if abs_qp_delta == 0 {
        return Ok(0);
    }

    // 4. Decode the Sign Flag (Bypass) - Only if abs > 0
    // In HEVC: 0 = Positive, 1 = Negative
    let sign_flag = ctx.cabac.decode_bypass();
    let final_delta = if sign_flag == 1 { -(abs_qp_delta as i32) } else { abs_qp_delta as i32 };

    debug_more!("Final cu_qp_delta: {}", final_delta);
    Ok(final_delta)
}
pub fn decode_quantization_parameters(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log2_cb_size: u8
) {
    debug_more!(
        "------------------decode_quantization_parameters(xc={},yc={},log2_cb_size={})----------",
        x0,
        y0,
        log2_cb_size
    );
    #[rustfmt::skip]
    const TABLE_8_22: [i8; 58] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
        13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
        23, 24, 25, 26, 27, 28, 29, 29, 30, 31,
        32, 33, 33, 34, 34, 35, 35, 36, 36, 37,
        37, 38, 39, 40, 41, 42, 43, 44, 45, 46,
        47, 48, 49, 50, 51
    ];

    let pps = ctx.pps;
    let sps = ctx.sps;

    // 1. Identify the Quantization Group (QG) boundary
    let qg_size = 1 << pps.log2_min_cu_qp_delta_size;
    let x_qg = x0 & !(qg_size - 1);
    let y_qg = y0 & !(qg_size - 1);

    let slice_qp = (26 + pps.init_qp_minus26 + ctx.slice_header.slice_qp_delta) as i8;

    // 2. QG State Reset
    if x_qg != ctx.current_qg_x || y_qg != ctx.current_qg_y {
        if ctx.current_qg_x == usize::MAX {
            // Absolute start of the parsing process
            ctx.last_qp_in_previous_qg = slice_qp;
        } else {
            ctx.last_qp_in_previous_qg = ctx.last_qp_in_slice;
        }

        ctx.current_qg_x = x_qg;
        ctx.current_qg_y = y_qg;

        // We entered a new QG! Reset the delta flags!
        ctx.is_cu_qp_delta_coded = false;
        ctx.cu_qp_delta = 0;
    }

    // 3. Determine qp_prev
    let ctb_size = 1 << sps.log2_ctb_size_y;
    let slice_start_x = (ctx.slice_header.slice_segment_address as usize
        % sps.pic_width_in_ctbs_y as usize)
        * ctb_size;
    let slice_start_y = (ctx.slice_header.slice_segment_address as usize
        / sps.pic_width_in_ctbs_y as usize)
        * ctb_size;

    let is_first_qg_in_slice = x_qg == slice_start_x && y_qg == slice_start_y;

    // WPP check (if entropy sync is enabled, the start of every CTB row resets to slice_qp)
    let is_first_in_ctb_row = x_qg == 0 && y_qg.is_multiple_of(ctb_size);

    let qp_prev =
        if is_first_qg_in_slice || (is_first_in_ctb_row && pps.entropy_coding_sync_enabled_flag) {
            slice_qp
        } else {
            ctx.last_qp_in_previous_qg
        };

    // 4. Derive Luma QP (QpY)
    let ctb_mask = ctb_size - 1;

    // Spatial prediction cannot cross CTU boundaries
    let qp_a = if x_qg == 0 || (x_qg & ctb_mask) == 0 {
        qp_prev
    } else {
        ctx.neighbor_tracker
            .get_qp_left(x_qg, y_qg)
            .unwrap_or(qp_prev)
    };

    let qp_b = if y_qg == 0 || (y_qg & ctb_mask) == 0 {
        qp_prev
    } else {
        ctx.neighbor_tracker
            .get_qp_above(x_qg, y_qg)
            .unwrap_or(qp_prev)
    };

    let qp_pred = (qp_a + qp_b + 1) >> 1;

    debug_more!(
        "QP Prediction: Left={}, Above={}, Prev={} -> Final Pred={}",
        if x_qg == 0 || (x_qg & ctb_mask) == 0 {
            "None".to_string()
        } else {
            format!("{:?}", ctx.neighbor_tracker.get_qp_left(x_qg, y_qg))
        },
        if y_qg == 0 || (y_qg & ctb_mask) == 0 {
            "None".to_string()
        } else {
            format!("{:?}", ctx.neighbor_tracker.get_qp_above(x_qg, y_qg))
        },
        qp_prev,
        qp_pred
    );

    // 5. Calculate Bit Depth Offsets
    let qp_bd_offset_y = 6 * (i32::from(sps.bit_depth_luma) - 8);
    let qp_bd_offset_c = 6 * (i32::from(sps.bit_depth_chroma) - 8);

    // Calculate and return the new QP
    let qp_y = ((i32::from(qp_pred) + ctx.cu_qp_delta + 52 + 2 * qp_bd_offset_y)
        % (52 + qp_bd_offset_y))
        - qp_bd_offset_y;

    // Then apply the prime calculation
    ctx.qp_y_prime = qp_y + qp_bd_offset_y;

    // 6. Chroma Derivation (Includes missing slice header offsets!)
    let qp_i_cb =
        (qp_y + pps.cb_qp_offset as i32 + i32::from(ctx.slice_header.slice_cb_qp_offset))
            .clamp(-qp_bd_offset_c, 57);
    let qp_i_cr =
        (qp_y + pps.cr_qp_offset as i32 + i32::from(ctx.slice_header.slice_cr_qp_offset))
            .clamp(-qp_bd_offset_c, 57);

    let (qp_cb, qp_cr) = if sps.chroma_format == ChromaFormat::Yuv420 {
        let idx_cb = (qp_i_cb + qp_bd_offset_c).clamp(0, 57) as usize;
        let idx_cr = (qp_i_cr + qp_bd_offset_c).clamp(0, 57) as usize;
        (
            TABLE_8_22[idx_cb] - qp_bd_offset_c as i8,
            TABLE_8_22[idx_cr] - qp_bd_offset_c as i8
        )
    } else {
        (qp_i_cb as i8, qp_i_cr as i8)
    };

    ctx.qp_cb_prime = (i32::from(qp_cb) + qp_bd_offset_c).max(0);
    ctx.qp_cr_prime = (i32::from(qp_cr) + qp_bd_offset_c).max(0);

    // 7. Update Tracker State
    ctx.last_qp_in_slice = qp_y as i8;
    debug_more!(
        "FINAL QPs: Y'={} Cb'={} Cr'={}",
        ctx.qp_y_prime,
        ctx.qp_cb_prime,
        ctx.qp_cr_prime
    );
    ctx.neighbor_tracker
        .update_qp(x0, y0, 1 << log2_cb_size, qp_y as i8);
}
