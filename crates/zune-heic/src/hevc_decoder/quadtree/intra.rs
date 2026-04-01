use crate::hevc_decoder::DEBUG_MORE;
use crate::debug_more;
use crate::hevc_decoder::cabac_tables::{CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE, CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG};
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
pub fn decode_prev_intra_luma_pred_flag(ctx:&mut DecodeSliceContext) -> u8 {
    debug_more!("prev_intra_luma_pred_flag");
    let bit = ctx.cabac.decode_decision(CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG);
    debug_more!("prev_intra_luma_pred_flag={}",bit);
    return bit;

}
pub fn decode_intra_luma_mode(
    ctx: &mut DecodeSliceContext,
    x0: usize, y0: usize,
    is_mpm: bool // Flag is now passed in
) -> u8 {
    let mpm_list = ctx.neighbor_tracker.derive_mpms(x0, y0);

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
        debug_more!(" mpm_idx={} mpm_list[{}]={}", mpm_idx,mpm_idx, mpm_list[mpm_idx]);

        mpm_list[mpm_idx]
    } else {
        debug_more!("rem_intra_luma_pred_mode (5 bits)");
        // rem_intra_luma_pred_mode: 5 bits fixed-length bypass
        let rem_mode = ctx.cabac.decode_fl_bypass(5) as u8;

        debug_more!("rem_intra_luma_pred_mode={}",rem_mode);
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
    // 1. Decode the DM (Derived Mode) flag
    // Context index for this is usually CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE
    let is_dm = ctx.cabac.decode_decision(CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE) == 0;

    if is_dm {
        debug_more!("Chroma Mode: Derived (DM) -> Mode {}", luma_mode);
        return luma_mode;
    }

    // 2. Decode the 3-bit bypass index (fixed-length 3 bits)
    let chroma_idx = ctx.cabac.decode_fl_bypass(3) as u8;

    // 3. Mapping Table (Spec Section 7.4.8.5)
    // The candidate modes are: Planar (0), Vertical (26), Horizontal (10), DC (1), and Mode 34.
    // Pick the "base" mode from the list based on our 3-bit index
    // Note: The index in the bitstream only goes up to 4.
    let mut selected_mode = match chroma_idx {
        0 => 0,  // Planar
        1 => 26, // Vertical
        2 => 10, // Horizontal
        3 => 1,  // DC
        4 => 34, // Mode 34
        _ => 34, // Safety fallback
    };

    // 4. Remapping Rule:
    // If our selected mode is the SAME as the luma mode,
    // we use the last candidate (Mode 34) instead.
    if selected_mode == luma_mode {
        selected_mode = 34;
    }

    debug_more!(
        "Chroma Mode: Signaled (idx {}), Final Mode: {} (Luma was {})",
        chroma_idx, selected_mode, luma_mode
    );

    selected_mode
}
