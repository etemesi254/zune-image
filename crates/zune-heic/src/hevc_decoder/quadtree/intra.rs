use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE, CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG
};
use crate::hevc_decoder::ctx::DecodeSliceContext;

// h.265-V2 Table 8-3
#[rustfmt::skip]
const MAP_CHROMA_422: [u8; 35] = [
    0, 1, 2, 2, 2, 2, 3,
    5, 7, 8, 10, 12, 13, 15,
    17, 18, 19, 20, 21, 22, 23,
    23, 24, 24, 25, 25, 26, 27,
    27, 28, 28, 29, 29, 30, 31
];
pub fn decode_prev_intra_luma_pred_flag(ctx: &mut DecodeSliceContext) -> u8 {
    debug_more!("prev_intra_luma_pred_flag");
    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG);
    debug_more!("prev_intra_luma_pred_flag={}", bit);
    return bit;
}
pub fn decode_intra_luma_mode(
    ctx: &mut DecodeSliceContext,
    x0: usize,
    y0: usize,
    is_mpm: bool
) -> u8 {

    let log2_ctu_size = ctx.sps.log2_min_luma_coding_block_size  + ctx.sps.log2_diff_max_min_luma_coding_block_size;
    let ctu_size = 1 << log2_ctu_size;
    let mpm_list = ctx.neighbor_tracker.derive_mpms(x0, y0,ctu_size);

    if is_mpm {
        debug_more!("MPM_IDX (TU:2)");

        // mpm_idx: Truncated Unary, cMax=2, bypass coded
        // bit 0 -> index 0
        // bit 1,0 -> index 1
        // bit 1,1 -> index 2
        let mpm_idx = if ctx.cabac.decode_bypass() == 0 {
            0
        } else if ctx.cabac.decode_bypass() == 0 {
            1
        } else {
            2
        };
        debug_more!(
            " mpm_idx={} mpm_list[{}]={}",
            mpm_idx,
            mpm_idx,
            mpm_list[mpm_idx]
        );

        mpm_list[mpm_idx]
    } else {
        debug_more!("rem_intra_luma_pred_mode (5 bits)");
        // rem_intra_luma_pred_mode: 5 bits fixed-length bypass
        let rem_mode = ctx.cabac.decode_fl_bypass(5) as u8;

        debug_more!("rem_intra_luma_pred_mode={}", rem_mode);
        let mut final_mode = rem_mode;
        let mut sorted_mpm = mpm_list;
        sorted_mpm.sort();

        for i in 0..3 {
            if final_mode >= sorted_mpm[i] {
                final_mode += 1;
            }
        }
        final_mode
    }
}

pub fn decode_intra_chroma_mode(ctx: &mut DecodeSliceContext, luma_mode: u8) -> u8 {
    // 1. Decode the prefix (Context-coded)
    // Spec: 0 = DM, 1 = Other
    let prefix = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE);

    if prefix == 0 {
        // Mode 4 in the syntax table corresponds to DM (Derived Mode)
        debug_more!("Chroma Mode: DM -> {}", luma_mode);
        return 4;
    }

    // 2. Decode the suffix (2 bits bypass)
    // This gives us an index 0..3
    let chroma_idx = ctx.cabac.decode_fl_bypass(2) as u8;

    debug_more!("Chroma Mode: Signaled (idx {}), (Luma: {})", chroma_idx,luma_mode);

    chroma_idx
}
