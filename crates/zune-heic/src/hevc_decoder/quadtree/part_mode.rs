use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_PART_MODE;
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::ctx::DecodeSliceContext;

pub fn decode_part_mode(ctx: &mut DecodeSliceContext, log2_cb_size: u8) -> PartMode {
    let sps = ctx.sps;

    if ctx.is_intra {
        // --- INTRA CASE ---
        // 1 bit: 1 = 2Nx2N, 0 = NxN
        // Only 8x8 (MinCbSize) is allowed to split into NxN for Intra
        let bit = ctx.cabac.decode_decision(CONTEXT_MODEL_PART_MODE);
        let mode = if bit == 1 { PartMode::Part2Nx2N } else { PartMode::PartNxN };

        debug_more!("part_mode (INTRA): {:?}", mode);
        mode
    } else {
        // --- INTER CASE ---
        // Bit 0: Is it 2Nx2N?
        let bit0 = ctx.cabac.decode_decision(CONTEXT_MODEL_PART_MODE);
        if bit0 == 1 {
            return PartMode::Part2Nx2N;
        }

        // Bit 1: Direction of split (1 = Horizontal, 0 = Vertical)
        let bit1 = ctx.cabac.decode_decision(CONTEXT_MODEL_PART_MODE + 1);

        if log2_cb_size > sps.log2_min_luma_coding_block_size {
            // Larger than minimum CU: check for AMP
            if sps.amp_enabled_flag {
                // Bit 3: Is it a symmetric split?
                let bit3 = ctx.cabac.decode_decision(CONTEXT_MODEL_PART_MODE + 3);
                if bit3 == 1 {
                    return if bit1 == 1 { PartMode::Part2NxN } else { PartMode::PartNx2N };
                }

                // Bit 4: Asymmetric split direction (Bypass coded)
                let bit4 = ctx.cabac.decode_bypass();
                if bit1 == 1 {
                    if bit4 == 1 { PartMode::Part2NxnD } else { PartMode::Part2NxnU }
                } else if bit4 == 0 { PartMode::PartnLx2N } else { PartMode::PartnRx2N }
            } else {
                if bit1 == 1 { PartMode::Part2NxN } else { PartMode::PartNx2N }
            }
        } else {
            // Minimum CU size (usually 8x8)
            if bit1 == 1 {
                return PartMode::Part2NxN;
            }

            // Smallest blocks (8x8) can split into NxN for Inter
            if log2_cb_size == 3 {
                PartMode::PartNx2N
            } else {
                // Bit 2: Nx2N vs NxN
                let bit2 = ctx.cabac.decode_decision(CONTEXT_MODEL_PART_MODE + 2);
                if bit2 == 1 { PartMode::PartNx2N } else { PartMode::PartNxN }
            }
        }
    }
}
