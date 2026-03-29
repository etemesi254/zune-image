use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac::CabacDecoder;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_CBF_CHROMA, CONTEXT_MODEL_CBF_LUMA, CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG,
    CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG, CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG,
    CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX,
    CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX, CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG,
    CONTEXT_MODEL_SPLIT_TRANSFORM_FLAG
};
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::quadtree::DecodeSliceContext;
use crate::hevc_decoder::quadtree::quant::{decode_cu_qp_delta, decode_quantization_parameters};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Component {
    Luma,
    Cb,
    Cr
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
) {
    let mut cbf_luma = false;
    let mut split_flag = false;

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
    let mut split_flag = false;

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
                );
            }
        }
    } else {
        // 3. Leaf Node: Decode Luma CBF
        // Intra blocks at depth 0 ALWAYS have cbf_luma = 1 if not explicitly split.
        if ctx.is_intra || trafo_depth != 0 || cbf_cb || cbf_cr {
            cbf_luma = decode_cbf_luma(ctx, trafo_depth);
        } else {
            cbf_luma = true; // Inferred
        }

        // 4. Enter the Transform Unit (Residual Coding)
        read_transform_unit(ctx, x0, y0, log2_trafo_size, cbf_luma, cbf_cb, cbf_cr);
    }
}

fn read_transform_unit(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log2_size: u8, cbf_luma: bool,
    cbf_cb: bool, cbf_cr: bool
) {
    debug_more!("---read_transform_unit(x0={},y0={},log2size={})", x0, y0, log2_size);
    // 1. HEVC Spec §7.3.8.11: cu_qp_delta is decoded here if enabled
    // and not yet coded for the current Quantization Group (QG).
    if (cbf_luma || cbf_cb || cbf_cr) && ctx.pps.cu_qp_delta_enabled_flag {
        if !ctx.is_cu_qp_delta_coded {
            ctx.cu_qp_delta = decode_cu_qp_delta(ctx);
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
    // Chroma blocks ALWAYS use DCT-II
    if cbf_cb {
        decode_residual_block(ctx, x0, y0, log2_size - 1, Component::Cb, false);
    }
    if cbf_cr {
        decode_residual_block(ctx, x0, y0, log2_size - 1, Component::Cr, false);
    }
}
fn decode_last_sig_pos(ctx: &mut DecodeSliceContext, log2_size: u8, comp: Component) -> (u32, u32) {
    let ctx_offset = if comp == Component::Luma {
        3 * (log2_size - 2) + ((log2_size - 1) >> 2)
    } else {
        15
    };

    let ctx_shift = if comp == Component::Luma { (log2_size - 2) >> 2 } else { log2_size - 2 };
    let max_prefix = (log2_size << 1) - 1;

    // Use specific X and Y bases from  table
    let last_x = decode_pos_component(
        ctx,
        CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX,
        max_prefix,
        ctx_offset,
        ctx_shift,
        "X"
    );
    let last_y = decode_pos_component(
        ctx,
        CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX,
        max_prefix,
        ctx_offset,
        ctx_shift,
        "Y"
    );

    (last_x, last_y)
}

fn decode_pos_component(
    ctx: &mut DecodeSliceContext, base_idx: usize, max_prefix: u8, ctx_offset: u8, ctx_shift: u8,
    label: &str
) -> u32 {
    let mut prefix = 0u32;
    while prefix < max_prefix as u32 {
        let ctx_idx = base_idx + (ctx_offset + (prefix as u8 >> ctx_shift)) as usize;
        let bin = ctx.cabac.decode_decision(ctx_idx);

        debug_more!("  LastPos_{} prefix_bin: {} (ctx: {})", label, bin, ctx_idx);

        if bin == 0 {
            break;
        }
        prefix += 1;
    }

    if prefix <= 3 {
        prefix
    } else {
        // Correct suffix math for prefixes 4 and above
        let suffix_len = (prefix >> 1) - 1;
        let suffix = ctx.cabac.decode_fl_bypass(suffix_len as u8);

        let base = (2 + (prefix & 1)) << suffix_len;
        let final_val = base + suffix;

        debug_more!(
            "  LastPos_{} suffix: {} (len: {}), final: {}",
            label,
            suffix,
            suffix_len,
            final_val
        );
        final_val
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ScanType {
    Diagonal = 0,
    Horizontal = 1,
    Vertical = 2
}

/// HEVC Spec §8.6.3: Derivation process for scan order
pub fn get_scan_type(log2_size: u8, is_intra: bool, luma_mode: u8) -> ScanType {
    // Only 4x4 (log2=2) and 8x8 (log2=3) Intra blocks can use non-diagonal scans.
    // Everything else (Inter, or Intra > 8x8) defaults to Diagonal.
    if is_intra && log2_size <= 3 {
        // Vertical-ish modes (6 to 14) use Vertical Scan
        if (6..=14).contains(&luma_mode) {
            debug_more!("Scan Order: Vertical (Mode {})", luma_mode);
            return ScanType::Vertical;
        }
        // Horizontal-ish modes (22 to 30) use Horizontal Scan
        else if (22..=30).contains(&luma_mode) {
            debug_more!("Scan Order: Horizontal (Mode {})", luma_mode);
            return ScanType::Horizontal;
        }
    }

    debug_more!(
        "Scan Order: Diagonal (log2_size: {}, Intra: {})",
        log2_size,
        is_intra
    );
    ScanType::Diagonal
}
#[derive(Clone, Copy, Debug)]
pub struct Pos {
    pub x: usize,
    pub y: usize
}

/// Pre-computed 4x4 Scan Tables (Spec §6.5.3)
#[rustfmt::skip]
const SCAN_4X4_DIAG: [Pos; 16] = [
    Pos { x: 0, y: 0 }, Pos { x: 0, y: 1 }, Pos { x: 1, y: 0 }, Pos { x: 0, y: 2 },
    Pos { x: 1, y: 1 }, Pos { x: 2, y: 0 }, Pos { x: 0, y: 3 }, Pos { x: 1, y: 2 },
    Pos { x: 2, y: 1 }, Pos { x: 3, y: 0 }, Pos { x: 1, y: 3 }, Pos { x: 2, y: 2 },
    Pos { x: 3, y: 1 }, Pos { x: 2, y: 3 }, Pos { x: 3, y: 2 }, Pos { x: 3, y: 3 },
];
#[rustfmt::skip]
const SCAN_4X4_HOR: [Pos; 16] = [
    Pos { x: 0, y: 0 }, Pos { x: 1, y: 0 }, Pos { x: 2, y: 0 }, Pos { x: 3, y: 0 },
    Pos { x: 0, y: 1 }, Pos { x: 1, y: 1 }, Pos { x: 2, y: 1 }, Pos { x: 3, y: 1 },
    Pos { x: 0, y: 2 }, Pos { x: 1, y: 2 }, Pos { x: 2, y: 2 }, Pos { x: 3, y: 2 },
    Pos { x: 0, y: 3 }, Pos { x: 1, y: 3 }, Pos { x: 2, y: 3 }, Pos { x: 3, y: 3 },
];
#[rustfmt::skip]
const SCAN_4X4_VER: [Pos; 16] = [
    Pos { x: 0, y: 0 }, Pos { x: 0, y: 1 }, Pos { x: 0, y: 2 }, Pos { x: 0, y: 3 },
    Pos { x: 1, y: 0 }, Pos { x: 1, y: 1 }, Pos { x: 1, y: 2 }, Pos { x: 1, y: 3 },
    Pos { x: 2, y: 0 }, Pos { x: 2, y: 1 }, Pos { x: 2, y: 2 }, Pos { x: 2, y: 3 },
    Pos { x: 3, y: 0 }, Pos { x: 3, y: 1 }, Pos { x: 3, y: 2 }, Pos { x: 3, y: 3 },
];
/// Pre-computed 8x8 Diagonal Scan Table (Spec §6.5.3)
/// Maps index 0..63 to (x, y) coordinates in an 8x8 grid.
/// Pre-computed 8x8 Diagonal Scan Table (Spec §6.5.3)
/// Maps index 0..63 to (x, y) coordinates in an 8x8 grid.
#[rustfmt::skip]
const SCAN_8X8_DIAG: [Pos; 64] = [
    Pos { x: 0, y: 0 }, Pos { x: 0, y: 1 }, Pos { x: 1, y: 0 }, Pos { x: 0, y: 2 },
    Pos { x: 1, y: 1 }, Pos { x: 2, y: 0 }, Pos { x: 0, y: 3 }, Pos { x: 1, y: 2 },
    Pos { x: 2, y: 1 }, Pos { x: 3, y: 0 }, Pos { x: 0, y: 4 }, Pos { x: 1, y: 3 },
    Pos { x: 2, y: 2 }, Pos { x: 3, y: 1 }, Pos { x: 4, y: 0 }, Pos { x: 0, y: 5 },
    Pos { x: 1, y: 4 }, Pos { x: 2, y: 3 }, Pos { x: 3, y: 2 }, Pos { x: 4, y: 1 },
    Pos { x: 5, y: 0 }, Pos { x: 0, y: 6 }, Pos { x: 1, y: 5 }, Pos { x: 2, y: 4 },
    Pos { x: 3, y: 3 }, Pos { x: 4, y: 2 }, Pos { x: 5, y: 1 }, Pos { x: 6, y: 0 },
    Pos { x: 0, y: 7 }, Pos { x: 1, y: 6 }, Pos { x: 2, y: 5 }, Pos { x: 3, y: 4 },
    Pos { x: 4, y: 3 }, Pos { x: 5, y: 2 }, Pos { x: 6, y: 1 }, Pos { x: 7, y: 0 },
    Pos { x: 1, y: 7 }, Pos { x: 2, y: 6 }, Pos { x: 3, y: 5 }, Pos { x: 4, y: 4 },
    Pos { x: 5, y: 3 }, Pos { x: 6, y: 2 }, Pos { x: 7, y: 1 }, Pos { x: 2, y: 7 },
    Pos { x: 3, y: 6 }, Pos { x: 4, y: 5 }, Pos { x: 5, y: 4 }, Pos { x: 6, y: 3 },
    Pos { x: 7, y: 2 }, Pos { x: 3, y: 7 }, Pos { x: 4, y: 6 }, Pos { x: 5, y: 5 },
    Pos { x: 6, y: 4 }, Pos { x: 7, y: 3 }, Pos { x: 4, y: 7 }, Pos { x: 5, y: 6 },
    Pos { x: 6, y: 5 }, Pos { x: 7, y: 4 }, Pos { x: 5, y: 7 }, Pos { x: 6, y: 6 },
    Pos { x: 7, y: 5 }, Pos { x: 6, y: 7 }, Pos { x: 7, y: 6 }, Pos { x: 7, y: 7 },
];

pub fn get_scan_table(scan_type: ScanType) -> &'static [Pos; 16] {
    match scan_type {
        ScanType::Diagonal => &SCAN_4X4_DIAG,
        ScanType::Horizontal => &SCAN_4X4_HOR,
        ScanType::Vertical => &SCAN_4X4_VER
    }
}
pub fn get_cg_pos(i: usize, log2_size: u8) -> Pos {
    match log2_size {
        // 4x4 block: only one CG at (0,0)
        2 => Pos { x: 0, y: 0 },

        // 8x8 block: 2x2 grid of CGs (indices 0..3)
        // Uses the first 4 entries of the 4x4 diagonal table
        3 => SCAN_4X4_DIAG[i],

        // 16x16 block: 4x4 grid of CGs (indices 0..15)
        4 => SCAN_4X4_DIAG[i],

        // 32x32 block: 8x8 grid of CGs (indices 0..63)
        5 => SCAN_8X8_DIAG[i],

        _ => {
            debug_more!("Warning: Invalid transform size log2={}", log2_size);
            Pos { x: 0, y: 0 }
        }
    }
}
/// Finds the index of the 4x4 Coefficient Group containing the coordinate (x, y).
/// x, y: coordinates of the last significant coefficient.
/// log2_size: log2 of the Transform Block size (e.g., 3 for 8x8, 5 for 32x32).
pub fn find_cg_index(x: u32, y: u32, log2_size: u8) -> usize {
    // 1. Convert pixel coordinates to CG coordinates (divide by 4)
    let cg_x = x >> 2;
    let cg_y = y >> 2;

    // 2. The CG scan is always Diagonal for blocks > 8x8 in many implementations,
    // but strictly follows the scan_type of the block.
    // For simplicity in finding the starting index, we can use the Z-scan
    // properties or just a simple mathematical mapping for common sizes.

    // In a TB of size (1 << log2_size), there are (1 << (log2_size - 2)) CGs per side.
    let width_in_cgs = 1 << (log2_size - 2);

    // To find where (cg_x, cg_y) sits in the scan order, we usually
    // use a pre-computed inverse map or a simple search since
    // the number of CGs is small (max 64 for a 32x32 block).

    // Here is the logic for a Diagonal Scan:
    let mut index = 0;
    let total_cgs = width_in_cgs * width_in_cgs;

    // In actual decoding, you often don't 'search'; you just know that
    // the loop starts at the CG containing (last_x, last_y) and ends at 0.
    // This helper calculates that 1D starting point.
    for i in 0..total_cgs {
        let pos = get_cg_pos(i, log2_size);
        if pos.x == cg_x as usize && pos.y == cg_y as usize {
            index = i;
            break;
        }
    }

    debug_more!("Last Coeff [{},{}] belongs to CG index: {}", x, y, index);
    index
}
fn get_sig_cg_ctx(
    cg_x: usize,
    cg_y: usize,
    log2_size: u8,
    comp: Component,
    sig_cg_flags: &[u8] // Your local 2D map for the current TB
) -> usize {
    let width_in_cgs = 1 << (log2_size - 2);

    // Look at neighbor CG flags: Right and Below
    let sig_l = if cg_x < width_in_cgs - 1 {
        sig_cg_flags[cg_y * width_in_cgs + (cg_x + 1)]
    } else {
        0
    };
    let sig_a = if cg_y < width_in_cgs - 1 {
        sig_cg_flags[(cg_y + 1) * width_in_cgs + cg_x]
    } else {
        0
    };

    let ctx_inc = (sig_l | sig_a) as usize; // HEVC Spec: ctxInc is 1 if either neighbor is 1
    let base = if comp == Component::Luma { 0 } else { 2 };

    CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG + base + ctx_inc
}
fn get_sig_ctx_inc(
    x_in_cg: usize, // 0..3
    y_in_cg: usize, // 0..3
    cg_x: usize,
    cg_y: usize,
    log2_size: u8,
    comp: Component,
    sig_cg_flags: &[u8]
) -> usize {
    let width_in_cgs = 1 << (log2_size - 2);

    // 1. Handle 4x4 blocks (Special case: Spec Table 9-40)
    if log2_size == 2 {
        let scan_idx = y_in_cg * 4 + x_in_cg;
        return SCAN_IDX_4X4_LUMA[scan_idx] as usize; // Pre-computed table below
    }

    // 2. Handle Luma blocks (8x8, 16x16, 32x32)
    if comp == Component::Luma {
        let sig_l = if cg_x < width_in_cgs - 1 {
            sig_cg_flags[cg_y * width_in_cgs + (cg_x + 1)]
        } else {
            0
        };
        let sig_a = if cg_y < width_in_cgs - 1 {
            sig_cg_flags[(cg_y + 1) * width_in_cgs + cg_x]
        } else {
            0
        };

        // Determine context set (0, 1, or 2) based on neighbor CGs
        let ctx_set = if sig_l > 0 && sig_a > 0 {
            2
        } else if sig_l > 0 || sig_a > 0 {
            1
        } else {
            0
        };

        // Final increment depends on position inside the 4x4 (x+y) and the set
        let mut ctx_inc = (y_in_cg << 2) + x_in_cg; // Basic position-based
        if ctx_inc > 2 {
            ctx_inc = 1;
        } else if ctx_inc > 0 {
            ctx_inc = 2;
        } else {
            ctx_inc = 0;
        }

        // Luma has offsets based on block size (8x8 gets its own, 16/32 share)
        let size_offset = if log2_size == 3 { 9 } else { 21 };
        return (ctx_set * 3) + size_offset + ctx_inc;
    }

    // 3. Handle Chroma blocks
    let sig_l = if cg_x < width_in_cgs - 1 {
        sig_cg_flags[cg_y * width_in_cgs + (cg_x + 1)]
    } else {
        0
    };
    let sig_a = if cg_y < width_in_cgs - 1 {
        sig_cg_flags[(cg_y + 1) * width_in_cgs + cg_x]
    } else {
        0
    };
    let ctx_set = if sig_l > 0 && sig_a > 0 {
        2
    } else if sig_l > 0 || sig_a > 0 {
        1
    } else {
        0
    };

    let ctx_inc = (y_in_cg + x_in_cg).min(2);
    (ctx_set * 3) + 33 + ctx_inc
}

// Pre-computed table for 4x4 Luma context increments
#[rustfmt::skip]
const SCAN_IDX_4X4_LUMA: [u8; 16] = [
    0, 1, 4, 5,
    2, 3, 4, 5,
    6, 6, 8, 8,
    7, 7, 8, 8
];
fn decode_residual_block(
    ctx: &mut DecodeSliceContext,
    x0: usize,
    y0: usize, // Pixel coordinates of the block
    log2_size: u8,
    comp: Component,
    _use_dst: bool // DST flag (used in Inverse Transform)
) -> Vec<i32> {
    let size = 1 << log2_size;
    let mut coeffs = vec![0i32; size * size];
    let num_cgs_wide = 1 << (log2_size - 2);

    debug_more!(
        "--- decode_residual_block [{},{}] size:{} comp:{:?} ---",
        x0,
        y0,
        size,
        comp
    );

    // 1. Decode Last Significant Coefficient Position
    let (last_x, last_y) = decode_last_sig_pos(ctx, log2_size, comp);

    // 2. Setup Scan Tables
    let scan_type = get_scan_type(log2_size, ctx.is_intra, ctx.intra_mode_luma);
    let scan_4x4 = get_scan_table(scan_type); // Internal 4x4 pattern
    let scan_cg = get_scan_table(ScanType::Diagonal); // External CG pattern (always diagonal)

    // 3. State Tracking for CGs
    let last_cg_idx = find_cg_index(last_x, last_y, log2_size);
    let mut sig_cg_flags = vec![0u8; num_cgs_wide * num_cgs_wide];
    let mut rice_param = 0u32;
    let mut current_ctx_set = 0u8;
    let mut prev_cg_had_gt1 = false;

    // 4. Loop through Coefficient Groups (CG) in reverse scan order
    for i in (0..=last_cg_idx).rev() {
        let cg_pos = scan_cg[i];
        let (cg_x, cg_y) = (cg_pos.x, cg_pos.y);

        // Update the context set based on the previous CG's result
        current_ctx_set = derive_ctx_set(comp, i, last_cg_idx, current_ctx_set, prev_cg_had_gt1);

        // Decode Significant Coefficient Group Flag (Coded Sub-Block Flag)
        let mut sig_cg_flag = 1u8;
        if i > 0 && i < last_cg_idx {
            let cg_ctx = get_sig_cg_ctx(cg_x, cg_y, log2_size, comp, &sig_cg_flags);
            sig_cg_flag = ctx.cabac.decode_decision(cg_ctx);
        }
        sig_cg_flags[cg_y * num_cgs_wide + cg_x] = sig_cg_flag;

        if sig_cg_flag == 1 {
            let mut sig_map = [0u8; 16];
            let mut num_sig = 0;

            // 5. Decode Significant Coefficient Flags for each of the 16 positions
            for n in (0..16).rev() {
                let curr_x = (cg_x << 2) + scan_4x4[n].x;
                let curr_y = (cg_y << 2) + scan_4x4[n].y;

                // Skip positions beyond the last significant coefficient
                if curr_x as u32 > last_x || curr_y as u32 > last_y {
                    continue;
                }

                if curr_x as u32 == last_x && curr_y as u32 == last_y {
                    sig_map[n] = 1;
                } else {
                    let ctx_inc = get_sig_ctx_inc(
                        scan_4x4[n].x,
                        scan_4x4[n].y,
                        cg_x,
                        cg_y,
                        log2_size,
                        comp,
                        &sig_cg_flags
                    );
                    sig_map[n] = ctx
                        .cabac
                        .decode_decision(CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG + ctx_inc);
                }

                if sig_map[n] == 1 {
                    num_sig += 1;
                }
            }

            // 6. Decode Absolute Magnitudes and Signs
            if num_sig > 0 {
                let abs_levels =
                    decode_abs_levels(ctx, &sig_map, &mut rice_param, comp, current_ctx_set);

                // Track for the next CG's context set
                prev_cg_had_gt1 = abs_levels.iter().any(|&l| l > 1);

                let mut sig_idx = 0;
                for n in (0..16).rev() {
                    if sig_map[n] == 1 {
                        let final_x = (cg_x << 2) + scan_4x4[n].x;
                        let final_y = (cg_y << 2) + scan_4x4[n].y;

                        let sign = ctx.cabac.decode_bypass();
                        let level = abs_levels[sig_idx] as i32;

                        coeffs[final_y * size + final_x] = if sign == 1 { -level } else { level };

                        debug_more!(
                            "  Coeff [{},{}]: {}",
                            final_x,
                            final_y,
                            coeffs[final_y * size + final_x]
                        );
                        sig_idx += 1;
                    }
                }
            } else {
                prev_cg_had_gt1 = false;
            }
        } else {
            prev_cg_had_gt1 = false;
        }
    }

    coeffs
}

/// HEVC Spec §9.3.3.1.2: Derivation process for ctxSet
///
/// Returns:
///   Luma: 0, 1, 2, or 3
///   Chroma: 0 or 1
fn derive_ctx_set(
    comp: Component, cg_idx: usize, last_cg_idx: usize, current_ctx_set: u8, prev_cg_had_gt1: bool
) -> u8 {
    // 1. The first CG processed (the one with the last significant coeff)
    // always starts with ctxSet 0.
    if cg_idx == last_cg_idx {
        return 0;
    }

    // 2. Derive the next set based on the previous CG's results
    if comp == Component::Luma {
        // Luma Set Logic:
        // If previous CG had a coeff > 1, we move "up" the sets (max 3).
        // If not, we stay at or move "down" to set 0 or 1.
        if prev_cg_had_gt1 {
            match current_ctx_set {
                0 => 2,
                1 => 3,
                _ => current_ctx_set // Stay at 2 or 3
            }
        } else {
            if current_ctx_set > 0 { 1 } else { 0 }
        }
    } else {
        // Chroma Set Logic:
        // Simple binary toggle. Set 0 (normal) or Set 1 (busy).
        if prev_cg_had_gt1 { 1 } else { 0 }
    }
}
fn decode_abs_levels(
    ctx: &mut DecodeSliceContext,
    sig_map: &[u8; 16],
    rice_param: &mut u32,
    comp: Component,
    ctx_set: u8 // Now passed in from the loop
) -> Vec<u32> {
    let num_sig = sig_map.iter().filter(|&&s| s == 1).count();
    let mut abs_levels = vec![1u32; num_sig];

    // 1. Pass 1: Decode coeff_abs_level_greater1_flag (G1)
    let mut num_greater1 = 0;
    let mut first_greater1_idx = -1i32;

    for i in 0..num_sig.min(8) {
        let ctx_idx = CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG
            + (ctx_set as usize * 4)
            + num_greater1.min(3);
        let bin = ctx.cabac.decode_decision(ctx_idx);

        if bin == 1 {
            abs_levels[i] += 1;
            num_greater1 += 1;
            if first_greater1_idx == -1 {
                first_greater1_idx = i as i32;
            }
        } else if num_greater1 > 0 && num_greater1 < 3 {
            num_greater1 += 1;
        }
    }

    // 2. Pass 2: Decode coeff_abs_level_greater2_flag (G2)
    if first_greater1_idx != -1 {
        let ctx_idx = CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG + ctx_set as usize;
        let bin = ctx.cabac.decode_decision(ctx_idx);

        if bin == 1 {
            abs_levels[first_greater1_idx as usize] += 1;
        }
    }

    // 3. Pass 3: Decode Remaining (Bypass)
    for i in 0..num_sig {
        let base_level = if i < 8 {
            if i as i32 == first_greater1_idx && abs_levels[i] == 3 {
                3
            } else if abs_levels[i] == 2 {
                2
            } else {
                1
            }
        } else {
            1
        };

        if abs_levels[i] == base_level
            && (i >= 8
                || abs_levels[i] >= 3
                || (i as i32 != first_greater1_idx && abs_levels[i] >= 2))
        {
            let remaining = decode_remaining_level(&mut ctx.cabac, *rice_param);
            abs_levels[i] = base_level + remaining;

            // Update Rice Parameter adaptation
            if abs_levels[i] > (3 << *rice_param) && *rice_param < 4 {
                *rice_param += 1;
            }
        }
    }

    abs_levels
}
/// HEVC Spec §9.3.3.1.3: Decoding process for coeff_abs_level_remaining
///
/// k: The current Rice parameter (0..4)
pub fn decode_remaining_level(cabac: &mut CabacDecoder, k: u32) -> u32 {
    // 1. Decode Prefix (Unary Bypass)
    // We count the number of 1-bits until we hit a 0.
    let mut prefix = 0u32;
    while cabac.decode_bypass() == 1 {
        prefix += 1;
        // Safety: HEVC escape codes shouldn't exceed the bit depth limits.
        if prefix > 24 {
            break;
        }
    }

    if prefix < 3 {
        // --- Case A: Golomb-Rice ---
        // Suffix is exactly 'k' bits.
        let suffix = cabac.decode_fl_bypass(k as u8);
        (prefix << k) + suffix
    } else {
        // --- Case B: Exp-Golomb ---
        // Suffix length increases as the prefix gets larger.
        let suffix_len = prefix - 3 + k;

        // Safety check to prevent overflow on malformed bits
        if suffix_len >= 32 {
            return 0;
        }

        let suffix = cabac.decode_fl_bypass(suffix_len as u8);

        // Formula: codeVal = (((1 << (prefix - 3)) + 3 - 1) << k) + suffix
        let base = ((1 << (prefix - 3)) + 2) << k;
        base + suffix
    }
}
