use crate::hevc_decoder::DEBUG_MORE;
use crate::debug_more;
use crate::hevc_decoder::cabac_tables::{CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE, CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG};
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::quadtree::DecodeSliceContext;
// h.265-V2 Table 8-3
#[rustfmt::skip]
const MAP_CHROMA_422: [u8; 35] = [
    0, 1, 2, 2, 2, 2, 3,
    5, 7, 8, 10, 12, 13, 15,
    17, 18, 19, 20, 21, 22, 23,
    23, 24, 24, 25, 25, 26, 27,
    27, 28, 28, 29, 29, 30, 31
];
pub fn decode_intra_luma_mode(ctx: &mut DecodeSliceContext, x0: usize, y0: usize) -> u8 {
    let mpm_list = ctx.neighbor_tracker.derive_mpms(x0, y0);

    // prev_intra_luma_pred_flag uses Context 0 of its model range
    let is_mpm = ctx.cabac.decode_decision(CONTEXT_MODEL_PREV_INTRA_LUMA_PRED_FLAG) == 1;

    if is_mpm {
        // mpm_idx is bypass coded (Truncated Unary, cMax=2)
        let mpm_idx = if ctx.cabac.decode_bypass() == 0 {
            0
        } else if ctx.cabac.decode_bypass() == 0 {
            1
        } else {
            2
        };

        let mode = mpm_list[mpm_idx];
        debug_more!("MPM Mode found at index {}: Mode {}", mpm_idx, mode);
        mode
    } else {
        // rem_intra_luma_pred_mode is 5 bits bypass (Fixed Length)
        let mut rem_mode = 0u8;
        for _ in 0..5 {
            rem_mode = (rem_mode << 1) | ctx.cabac.decode_bypass() as u8;
        }

        let mut final_mode = rem_mode;
        let mut sorted_mpm = mpm_list;
        sorted_mpm.sort();

        // Re-insertion: skip over the 3 modes that were in the MPM list
        for i in 0..3 {
            if final_mode >= sorted_mpm[i] {
                final_mode += 1;
            }
        }

        debug_more!("Remaining Mode Decoded: {} (raw: {})", final_mode, rem_mode);
        final_mode
    }
}


pub fn decode_intra_chroma_mode(ctx: &mut DecodeSliceContext, luma_mode: u8) -> u8 {
    // 1. Decode the intra_chroma_pred_mode index
    // Bin 0: Context-coded (Base 13). 0 = DM, 1 = Not DM.
    let bin0 = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_INTRA_CHROMA_PRED_MODE);

    let chroma_idx = if bin0 == 0 {
        4 // DM_CHROMA (Derived Mode)
    } else {
        // Bins 1 and 2: Bypass-coded. These form a 2-bit index (0-3).
        (ctx.cabac.decode_bypass() << 1) | ctx.cabac.decode_bypass()
    };

    // 2. Map the signaled index to a base mode
    let mut chroma_mode = match chroma_idx {
        0 => 0,         // Planar
        1 => 26,        // Vertical
        2 => 10,        // Horizontal
        3 => 1,         // DC
        4 => luma_mode, // Derived from Luma
        _ => unreachable!()
    };

    // 3. Apply the "Conflict" Rule (Spec Table 7-3)
    // If the signaled mode (0-3) is identical to the Luma mode,
    // it is remapped to Mode 34 (Intra_Angular 34) to avoid redundancy.
    if chroma_idx < 4 && chroma_mode == luma_mode {
        chroma_mode = 34;
    }

    // 4. Handle 4:2:2 Chroma Format
    // In 4:2:2, the vertical resolution is double that of 4:2:0 relative to the width.
    // We must remap the angles so they look correct in the stretched space.
    if ctx.sps.chroma_format == ChromaFormat::Yuv422 {
        let original_mode = chroma_mode;
        chroma_mode = MAP_CHROMA_422[chroma_mode as usize];
        debug_more!("4:2:2 Chroma Remap: {} -> {}", original_mode, chroma_mode);
    }

    debug_more!(
        "Final Chroma Mode: signaled_idx={}, mode={} (Luma was {})",
        chroma_idx,
        chroma_mode,
        luma_mode
    );

    chroma_mode
}

