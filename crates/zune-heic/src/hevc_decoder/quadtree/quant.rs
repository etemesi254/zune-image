use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::quadtree::DecodeSliceContext;

pub fn decode_cu_qp_delta(ctx: &mut DecodeSliceContext) -> i32 {
    debug_more!("decode_cu_qp_delta");
    // HEVC uses 2 contexts for cu_qp_delta_abs:
    // ctxIdx = 0 for the first bin
    // ctxIdx = 1 for the remaining bins (1 to 4)
    // We assume these are defined in your CABAC model offset.
    let ctx_base = 0; // Placeholder for the actual QP delta context offset

    // 1. Decode the Prefix (Truncated Unary, max length 5)
    let mut prefix = 0;
    while prefix < 5 {
        // Use context 0 for the first bin, context 1 for all subsequent prefix bins
        let ctx_idx = if prefix == 0 { ctx_base } else { ctx_base + 1 };
        let bin = ctx.cabac.decode_decision(ctx_idx);

        if bin == 0 {
            break;
        }
        prefix += 1;
    }


    let mut abs_qp_delta = prefix as u32;

    // 2. Decode the Suffix (Exp-Golomb order 0)
    // Only present if the prefix reached the maximum value of 5
    if prefix == 5 {
        let suffix_val = ctx.cabac.decode_bypass_eg0();
        abs_qp_delta += suffix_val;
        debug_more!(
            "QP Delta Suffix decoded: {}, Total Abs: {}",
            suffix_val,
            abs_qp_delta
        );
    }

    if abs_qp_delta == 0 {
        return 0;
    }

    // 3. Decode the Sign Flag (Bypass coded)
    // 0 = Positive, 1 = Negative
    let sign_flag = ctx.cabac.decode_bypass();
    let final_delta = if sign_flag == 1 { -(abs_qp_delta as i32) } else { abs_qp_delta as i32 };

    debug_more!("Final cu_qp_delta: {}", final_delta);
    panic!();
    final_delta
}
pub fn decode_quantization_parameters(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log2_cb_size: u8
) {
    debug_more!(
        "-------------decode_quantization_parameters( xc={},yc={}) ---------------",
        x0,
        y0
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

    debug_more!("x_qg={}, y_qg={}", x_qg, y_qg);
    // If we've moved to a new QG, reset the "already coded" flag
    if x_qg != ctx.current_qg_x || y_qg != ctx.current_qg_y {
        debug_more!(
            "New QG detected at [{}, {}]. Resetting cu_qp_delta_coded.",
            x_qg,
            y_qg
        );
        ctx.current_qg_x = x_qg;
        ctx.current_qg_y = y_qg;
        ctx.is_cu_qp_delta_coded = false;
    }

    // 2. Derive the Predictor (qP_pred)
    // This only happens once per QG. If already coded, we use the last derived QP.
    let qp_pred = if !ctx.is_cu_qp_delta_coded {
        let qp_a = ctx.neighbor_tracker.get_qp_left(x0, y0);
        let qp_b = ctx.neighbor_tracker.get_qp_above(x0, y0);

        let pred = match (qp_a, qp_b) {
            (Some(a), Some(b)) => (a + b + 1) >> 1,
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => ctx.last_qp_in_slice // Fallback to Slice QP or previous CU
        };

        debug_more!(
            "QP Prediction: Left={:?}, Above={:?}, Prev={} -> Final Pred={}",
            qp_a,
            qp_b,
            ctx.last_qp_in_slice,
            pred
        );
        pred
    } else {
        ctx.last_qp_in_slice
    };
    let qp_y = ((qp_pred as i32 + ctx.cu_qp_delta + 52) % 52) as i8;
    // 1. Calculate Bit Depth Offsets
    // QpBdOffset = 6 * (bit_depth - 8)
    let qp_bd_offset_y = 6 * (sps.bit_depth_luma as i32 - 8);
    let qp_bd_offset_c = 6 * (sps.bit_depth_chroma as i32 - 8);

    // 2. Final Luma QP Prime (Qp'Y)
    // qp_y is the value we derived earlier (0-51)
    ctx.qp_y_prime = (qp_y as i32) + qp_bd_offset_y;

    // 3. Calculate intermediate Chroma QP index (qPi)
    // Formula: qPi = Clip3(-QpBdOffset_C, 57, QpY + pps_offset + slice_offset + cu_offset)

    // Note: CuQpOffsetCb/Cr come from PPS extensions (usually 0 in Main profile)
    let cu_qp_offset_cb = 0;
    let cu_qp_offset_cr = 0;

    let qp_i_cb = (qp_y as i32 + pps.cb_qp_offset as i32 + 0 /* slice offset */ + cu_qp_offset_cb)
        .clamp(-qp_bd_offset_c, 57);
    let qp_i_cr = (qp_y as i32 + pps.cr_qp_offset as i32 + 0 /* slice offset */ + cu_qp_offset_cr)
        .clamp(-qp_bd_offset_c, 57);

    debug_more!(
        "qPiCb: {} (PPS: {}), qPiCr: {} (PPS: {})",
        qp_i_cb,
        pps.cb_qp_offset,
        qp_i_cr,
        pps.cr_qp_offset
    );

    // 4. Map to final Chroma QPs based on Chroma Format
    let (qp_cb, qp_cr) = if sps.chroma_format == ChromaFormat::Yuv420 {
        // Use non-linear table for 4:2:0
        // We offset by qp_bd_offset_c to index the table properly if it's 10-bit
        let idx_cb = (qp_i_cb + qp_bd_offset_c).clamp(0, 57) as usize;
        let idx_cr = (qp_i_cr + qp_bd_offset_c).clamp(0, 57) as usize;
        (
            TABLE_8_22[idx_cb] - qp_bd_offset_c as i8,
            TABLE_8_22[idx_cr] - qp_bd_offset_c as i8
        )
    } else {
        // 4:2:2 and 4:4:4 use linear mapping
        (qp_i_cb as i8, qp_i_cr as i8)
    };

    // 5. Calculate Final Chroma QP Primes (Qp'Cb and Qp'Cr)
    ctx.qp_cb_prime = (qp_cb as i32 + qp_bd_offset_c).max(0);
    ctx.qp_cr_prime = (qp_cr as i32 + qp_bd_offset_c).max(0);

    debug_more!(
        "FINAL QPs: Y'={} Cb'={} Cr'={}",
        ctx.qp_y_prime,
        ctx.qp_cb_prime,
        ctx.qp_cr_prime
    );

    // Update global state
    ctx.last_qp_in_slice = qp_y;
    ctx.neighbor_tracker
        .update_qp(x0, y0, 1 << log2_cb_size, qp_y);
}
