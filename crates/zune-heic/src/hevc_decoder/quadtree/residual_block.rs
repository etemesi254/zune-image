use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::{
    CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG, CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG,
    CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG,
    CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX,
    CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX, CONTEXT_MODEL_RDPCM_DIR,
    CONTEXT_MODEL_RDPCM_FLAG, CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG,
    CONTEXT_MODEL_TRANSFORM_SKIP_FLAG
};
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::neighbor_tracker::PredMode;
use crate::hevc_decoder::quadtree::transform_unit::Component;
use crate::hevc_decoder::quadtree::transform_unit::Component::Luma;

pub struct ScanPosition {
    pub scan_pos:  i32,
    pub sub_block: i32
}

#[derive(Copy, Clone, Debug)]
struct Pos {
    pub x: u8,
    pub y: u8
}

#[derive(Debug, Clone, Copy)]
pub struct Greater1State {
    pub bit:          u8,
    pub greater1_ctx: i32,
    pub ctx_set:      i32
}
// log2_size == 1 (2x2 grid)
const SCAN_2X2_DIAG:[Pos;4] = [
Pos { x: 0, y: 0 },
Pos { x: 0, y: 1 },
Pos { x: 1, y: 0 },
Pos { x: 1, y: 1 },
];

const SCAN_2X2_HOR:[Pos;4] = [
Pos { x: 0, y: 0 },
Pos { x: 1, y: 0 },
Pos { x: 0, y: 1 },
Pos { x: 1, y: 1 },
];

const SCAN_2X2_VER:[Pos;4] = [
Pos { x: 0, y: 0 },
Pos { x: 0, y: 1 },
Pos { x: 1, y: 0 },
Pos { x: 1, y: 1 },
];
// Static tables for 4x4 blocks (log2 = 2)
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

// Horizontal Scan: Row by row
const SCAN_8X8_HOR: [Pos; 64] = {
    let mut pos = [Pos { x: 0, y: 0 }; 64];
    let mut i = 0;
    while i < 64 {
        pos[i] = Pos {
            x: (i % 8) as u8,
            y: (i / 8) as u8
        };
        i += 1;
    }
    pos
};

// Vertical Scan: Column by column
const SCAN_8X8_VER: [Pos; 64] = {
    let mut pos = [Pos { x: 0, y: 0 }; 64];
    let mut i = 0;
    while i < 64 {
        pos[i] = Pos {
            x: (i / 8) as u8,
            y: (i % 8) as u8
        };
        i += 1;
    }
    pos
};

const SCAN_1X1: [Pos; 1] = [Pos { x: 0, y: 0 }];
fn get_scan_order(log2_size: u8, scan_idx: u8) -> &'static [Pos] {
    match (log2_size, scan_idx) {
        (0, _) => &SCAN_1X1,
        // --- 2x2 Sub-block Scans (log2_size = 1) ---
        (1, 0) => &SCAN_2X2_DIAG,
        (1, 1) => &SCAN_2X2_HOR,
        (1, 2) => &SCAN_2X2_VER,

        // --- 4x4 Scans (log2_size = 2) ---
        (2, 0) => &SCAN_4X4_DIAG,
        (2, 1) => &SCAN_4X4_HOR,
        (2, 2) => &SCAN_4X4_VER,

        // --- 8x8 Scans (log2_size = 3) ---
        (3, 0) => &SCAN_8X8_DIAG,
        (3, 1) => &SCAN_8X8_HOR,
        (3, 2) => &SCAN_8X8_VER,

        _ => panic!("Unsupported scan config: scan_idx={scan_idx}, log2size={log2_size}")
    }
}
pub fn decode_transform_skip_flag(ctx: &mut DecodeSliceContext, component: Component) -> u8 {
    // Context index is 0 for Luma (Y), and 1 for Chroma (Cb/Cr)
    let ctx_inc = usize::from(component != Component::Luma);

    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_TRANSFORM_SKIP_FLAG + ctx_inc);

    debug_more!("transform_skip_flag[{:?}] = {}", component, bit);
    bit
}

pub fn decode_explicit_rdpcm_flag(ctx: &mut DecodeSliceContext, component: Component) -> bool {
    // Context: 0 for Luma, 1 for Chroma
    let ctx_inc = usize::from(component != Component::Luma);
    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_RDPCM_FLAG + ctx_inc);

    debug_more!("explicit_rdpcm_flag[{:?}] = {}", component, bit);
    bit == 1
}

pub fn decode_explicit_rdpcm_dir(ctx: &mut DecodeSliceContext, component: Component) -> u8 {
    // Context: 0 for Luma, 1 for Chroma
    let ctx_inc = usize::from(component != Component::Luma);
    let bit = ctx.cabac.decode_decision(CONTEXT_MODEL_RDPCM_DIR + ctx_inc);

    debug_more!("explicit_rdpcm_dir[{:?}] = {}", component, bit);
    bit // 0 = Horizontal, 1 = Vertical
}
pub fn decode_last_significant_coeff_prefix(
    ctx: &mut DecodeSliceContext, log2_trafo_size: u8, c_idx: Component, is_x: bool
) -> u8 {
    debug_more!(
        "# last_significant_coeff_prefix log2TrafoSize:{} cIdx:{:?}",
        log2_trafo_size,
        c_idx
    );

    let c_max = (log2_trafo_size << 1) - 1;

    let ctx_offset: i32;
    let ctx_shift: u8;

    if c_idx == Component::Luma {
        // Luma formulas
        ctx_offset = 3 * (i32::from(log2_trafo_size) - 2) + ((i32::from(log2_trafo_size) - 1) >> 2);
        ctx_shift = (log2_trafo_size + 1) >> 2;
    } else {
        // Chroma formulas
        ctx_offset = 15;
        ctx_shift = log2_trafo_size - 2;
    }

    // Determine which base context to use
    let base_model_idx = if is_x {
        CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_X_PREFIX
    } else {
        CONTEXT_MODEL_LAST_SIGNIFICANT_COEFFICIENT_Y_PREFIX
    };

    let mut value = c_max;
    for bin_idx in 0..c_max {
        let ctx_idx_inc = (bin_idx >> ctx_shift) as usize;

        debug_more!("context: {}+{}", ctx_offset, ctx_idx_inc);

        let bit = ctx
            .cabac
            .decode_decision(base_model_idx + (ctx_offset as usize) + ctx_idx_inc);

        if bit == 0 {
            value = bin_idx;
            break;
        }
    }

    debug_more!("> last_significant_coeff_prefix: {}", value);
    value
}

pub fn get_intra_scan_idx(
    ctx: &DecodeSliceContext, log2_trafo_size: u8, intra_mode: u8, component: Component
) -> u8 {
    // Condition: 4x4 block OR (8x8 block AND (Luma OR 4:4:4 Chroma))
    let is_small_block = log2_trafo_size == 2;
    let is_8x8_optimized = log2_trafo_size == 3
        && (component == Component::Luma || ctx.sps.chroma_format == ChromaFormat::Yuv444);

    if is_small_block || is_8x8_optimized {
        // Vertical Scan (2)
        return if (6..=14).contains(&intra_mode) {
            2
        }
        // Horizontal Scan (1)
        else if (22..=30).contains(&intra_mode) {
            1
        }
        // Diagonal Scan (0)
        else {
            0
        };
    }
    // Larger blocks always use Diagonal Scan
    0
}
pub fn get_scan_position(x: u32, y: u32, scan_idx: u8, log2_trafo_size: u8) -> ScanPosition {
    // 1. Determine CG coordinates (the 4x4 blocks)
    let cg_x = (x >> 2) as u8;
    let cg_y = (y >> 2) as u8;

    // 2. Determine local coordinates inside the CG
    let pos_x = (x & 3) as u8;
    let pos_y = (y & 3) as u8;

    // 3. Get the tables from the spec (ScanOrder[log2TrafoSize-2][scanIdx])
    let scan_order_sub = get_scan_order(log2_trafo_size - 2, scan_idx);
    let scan_order_pos = get_scan_order(2, scan_idx);

    // 4. Search for the CG index (lastSubBlock)
    let mut sub_block = 0;
    for (i, pos) in scan_order_sub.iter().enumerate() {
        if pos.x == cg_x && pos.y == cg_y {
            sub_block = i as i32;
            break;
        }
    }

    // 5. Search for the internal scan index (lastScanPos)
    let mut scan_pos = 0;
    for (n, pos) in scan_order_pos.iter().enumerate() {
        if pos.x == pos_x && pos.y == pos_y {
            scan_pos = n as i32;
            break;
        }
    }

    ScanPosition {
        scan_pos,
        sub_block
    }
}
pub fn decode_coded_sub_block_flag(
    ctx: &mut DecodeSliceContext, component: Component, neighbor_info: u8
) -> bool {
    debug_more!("coded_sub_block_flag [{:?}] = {}", component, neighbor_info);
    // libde265 logic: ctxIdxInc = (neighbor_info > 0 ? 1 : 0)
    // neighbor_info is > 0 if either bit 1 (right) or bit 2 (bottom) is set.
    let ctx_inc = usize::from(neighbor_info > 0);

    // Luma contexts start at 0, Chroma at 2
    let ctx_base = if component == Luma { 0 } else { 2 };

    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_CODED_SUB_BLOCK_FLAG + ctx_base + ctx_inc);

    bit == 1
}

pub fn decode_significant_coeff_flag_lookup(ctx: &mut DecodeSliceContext, ctx_idx: usize) -> bool {
    debug_more!("decode_significant_coeff_flag_lookup [{:?}]", ctx_idx);

    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_SIGNIFICANT_COEFF_FLAG + ctx_idx);
    debug_more!(" > decode_significant_coeff_flag[{:?}] = {}", ctx_idx, bit);

    bit == 1
}

#[allow(clippy::too_many_arguments)]
fn decode_coeff_abs_level_greater1(
    ctx: &mut DecodeSliceContext,
    c_idx: usize,
    i: i32, // subblock index
    first_coeff_in_subblock: bool,
    first_subblock: bool,
    last_subblock_greater1_ctx: i32,
    prev_state: Greater1State, 
    c1: i32
) -> Greater1State {
    debug_more!("# coeff_abs_level_greater1");
    debug_more!(
        "  cIdx:{} i:{} firstCoeffInSB:{} firstSB:{} lastSB>1:{} last>1Ctx:{} lastLev>1:{} lastCtxSet:{}",
        c_idx,
        i,
        first_coeff_in_subblock,
        first_subblock,
        last_subblock_greater1_ctx,
        prev_state.greater1_ctx,
        prev_state.bit,
        prev_state.ctx_set
    );

    let mut greater1_ctx: i32;
    let mut ctx_set: i32;

    debug_more!("c1: {}", c1);

    if first_coeff_in_subblock {
        // --- Initialization for a new 4x4 sub-block ---
        // Logic for block with real DC -> ctx 0
        ctx_set = if i == 0 || c_idx > 0 { 0 } else { 2 };

        let last_greater1_ctx = if first_subblock { 1 } else { last_subblock_greater1_ctx };

        if last_greater1_ctx == 0 {
            ctx_set += 1;
        }

        debug_more!("ctxSet: {}", ctx_set);
        greater1_ctx = 1;
    } else {
        // --- Carry over state from previous coefficient in same sub-block ---
        ctx_set = prev_state.ctx_set;
        debug_more!("ctxSet (old): {}", ctx_set);

        greater1_ctx = prev_state.greater1_ctx;
        if greater1_ctx > 0 {
            if prev_state.bit == 1 {
                // If the previous coeff was > 1, we reset context to 0
                greater1_ctx = 0;
            } else {
                // Otherwise, increment the context (max 3, handled in ctx_idx_inc)
                greater1_ctx += 1;
            }
        }
    }

    // HM (Reference Software) algorithm override
    ctx_set = c1;

    // Calculate context index increment
    // Logic: (ctx_set * 4) + min(greater1_ctx, 3)
    let mut ctx_idx_inc = (ctx_set * 4) + (if greater1_ctx >= 3 { 3 } else { greater1_ctx });

    // Offset for Chroma components
    if c_idx > 0 {
        ctx_idx_inc += 16;
    }

    // CABAC Decode
    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER1_FLAG + ctx_idx_inc as usize);

    // Return the updated state in a struct
    Greater1State {
        bit,
        greater1_ctx,
        ctx_set
    }
}
fn decode_coeff_abss_level_greater2(
    ctx: &mut DecodeSliceContext, c_idx: usize, ctx_set: usize
) -> u8 {
    debug_more!(
        "decode_coeff_abss_level_greater2(c_idx={:?},ctx_set={})",
        ctx_set,
        c_idx
    );
    let idx = if c_idx > 0 { ctx_set + 4 } else { ctx_set };

    let bit = ctx
        .cabac
        .decode_decision(CONTEXT_MODEL_COEFF_ABS_LEVEL_GREATER2_FLAG + idx);

    debug_more!(
        " > decode_coeff_abss_level_greater2[{:?}] = {}",
        ctx_set,
        bit
    );

    bit
}
pub fn decode_coeff_abs_level_remaining(ctx: &mut DecodeSliceContext, c_rice_param: u8) -> i32 {
    debug_more!("# decode_coeff_abs_level_remaining");

    // 1. Decode Prefix (Unary bypass)
    let mut prefix = 0;
    while ctx.cabac.decode_bypass() == 1 {
        prefix += 1;
        if prefix > 32 {
            // Safety threshold
            return 0;
        }
    }

    // 2. Decode Suffix based on prefix length
    let value: i32;
    if prefix <= 3 {
        // Truncated Rice part
        let codeword = ctx.cabac.decode_fl_bypass(c_rice_param);
        value = (prefix << c_rice_param) + codeword as i32;
    } else {
        // Exp-Golomb part (prefix-3)
        // libde265 math: (((1 << (prefix-3)) + 2) << cRiceParam) + codeword
        let n_bits = (prefix - 3 + i32::from(c_rice_param)) as usize;
        let codeword = ctx.cabac.decode_fl_bypass(n_bits as u8);
        value = (((1 << (prefix - 3)) + 2) << c_rice_param) + codeword as i32;
    }

    debug_more!("$1 coeff_abs_level_remaining={}", value);
    value
}

#[allow(clippy::too_many_lines)]
pub fn decode_residual_block(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log2_trafo_size: u8, component: Component,
) {
    debug_more!(
        "--- residual_coding x0:{} y0:{} log2TrafoSize:{} cIdx:{:?}",
        x0,
        y0,
        log2_trafo_size,
        component
    );
    let pps = ctx.pps;

    let pred_mode = ctx.neighbor_tracker.get_pred_mode(x0, y0);

    if component == Component::Luma {
        ctx.neighbor_tracker
            .set_nonzero_coefficient(x0, y0, log2_trafo_size);
    }

    // --- Transform Skip Logic ---
    let transform_skip_flag ;
    let log2_max_transform_skip_size = if let Some(range) = pps.range_extension.as_ref() {
        range.log2_max_transform_skip_block_size
    } else {
        0
    };
    if ctx.pps.transform_skip_enabled_flag
        && !ctx.cu_transquant_bypass_flag
        && log2_trafo_size <= log2_max_transform_skip_size
    {
        transform_skip_flag = decode_transform_skip_flag(ctx, component);
    } else {
        transform_skip_flag = 0;
    }
    // Store in context for coefficient decoding and RDPCM
    ctx.transform_skip_flag[component as usize] = transform_skip_flag;

    ctx.explicit_rdpcm_flag = false;

    // --- Explicit RDPCM Logic ---
    // 3. Explicit RDPCM Logic (Range Extension)
    let mut explicit_rdpcm_flag = false;
    let mut explicit_rdpcm_dir = 0u8;

    // Use .as_ref() to safely check the Option<SpsRangeExtension>
    let rext_enabled = ctx
        .sps
        .range_extension
        .as_ref()
        .is_some_and(|re| re.explicit_rdpcm_enabled_flag);

    let ts_or_bypass = transform_skip_flag > 0 || ctx.cu_transquant_bypass_flag;

    if pred_mode == PredMode::ModeInter && rext_enabled && ts_or_bypass {
        explicit_rdpcm_flag = decode_explicit_rdpcm_flag(ctx, component);
        if explicit_rdpcm_flag {
            explicit_rdpcm_dir = decode_explicit_rdpcm_dir(ctx, component);
        }
    }

    ctx.explicit_rdpcm_flag = explicit_rdpcm_flag;
    ctx.explicit_rdpcm_dir = explicit_rdpcm_dir;

    // sb_type for persistent_rice_adaptation_enabled_flag
    // let mut sb_type = if component == Component::Luma { 2 } else { 0 };
    // if ctx.transform_skip_flag[component as usize] == 1 || ctx.cu_transquant_bypass_flag {
    //     sb_type += 1;
    // }
    let last_significant_coeff_x_prefix =
        decode_last_significant_coeff_prefix(ctx, log2_trafo_size, component, true);
    let last_significant_coeff_y_prefix =
        decode_last_significant_coeff_prefix(ctx, log2_trafo_size, component, false);

    // 1. Reconstruct LastSignificantCoeffX
    let last_significant_coeff_x: u32;

    if last_significant_coeff_x_prefix > 3 {
        let n_bits = (last_significant_coeff_x_prefix >> 1) - 1;
        let last_x_suffix = ctx.cabac.decode_fl_bypass(n_bits);

        last_significant_coeff_x =
            ((2 + (u32::from(last_significant_coeff_x_prefix) & 1)) << n_bits) + last_x_suffix;
    } else {
        last_significant_coeff_x = u32::from(last_significant_coeff_x_prefix);
    }

    // 2. Reconstruct LastSignificantCoeffY
    let last_significant_coeff_y: u32;
    if last_significant_coeff_y_prefix > 3 {
        let n_bits = (last_significant_coeff_y_prefix >> 1) - 1;
        let last_y_suffix = ctx.cabac.decode_fl_bypass(n_bits);

        last_significant_coeff_y =
            ((2 + (u32::from(last_significant_coeff_y_prefix) & 1)) << n_bits) + last_y_suffix;
    } else {
        last_significant_coeff_y = u32::from(last_significant_coeff_y_prefix);
    }

    // --- Determine scan_idx (libde265 style) ---
    let scan_idx = if pred_mode == PredMode::ModeIntra {
        let mode = if component == Component::Luma {
            ctx.neighbor_tracker.get_intra_mode(x0, y0)
        } else {
            ctx.intra_mode_chroma // Our decoded chroma mode
        };

        get_intra_scan_idx(ctx, log2_trafo_size, mode, component)
    } else {
        0 // Inter is always Diagonal
    };

    if pred_mode == PredMode::ModeIntra {
        if component == Component::Luma {
            debug_more!(
                "luma scan idx={} <- intra mode={}",
                scan_idx,
                ctx.neighbor_tracker.get_intra_mode(x0, y0)
            );
        } else {
            debug_more!(
                "chroma scan idx={} <- intra mode={} chroma:{:?} trsize:{}",
                scan_idx,
                ctx.intra_mode_chroma,
                ctx.sps.chroma_format,
                1 << log2_trafo_size
            );
        }
    }

    // --- Apply the Coordinate Swap ---
    // If scanIdx == 2 (Vertical), swap X and Y
    let mut last_sig_x = last_significant_coeff_x;
    let mut last_sig_y = last_significant_coeff_y;

    if scan_idx == 2 {
        std::mem::swap(&mut last_sig_x, &mut last_sig_y);
    }
    debug_more!(
        "LastSignificantCoeff: x={};y={}",
        last_significant_coeff_x,
        last_significant_coeff_y
    );

    // ScanOrderSub: Order of 4x4 sub-blocks within the TU
    // (Size depends on TU size: e.g., 16x16 TU has 4x4 sub-blocks)
    let scan_order_sub = get_scan_order(log2_trafo_size - 2, scan_idx);

    // ScanOrderPos: Order of coefficients within a 4x4 sub-block
    // (Always size 2, because sub-blocks are always 4x4)
    let scan_order_pos = get_scan_order(2, scan_idx);

    if DEBUG_MORE {
        // libde265 style tracing
        let mut scan_pos_trace = String::from("ScanOrderPos: ");
        for n in 0..16 {
            let pos = scan_order_pos[n];
            scan_pos_trace.push_str(&format!("({},{}) ", pos.x, pos.y));
        }
        debug_more!("{}", scan_pos_trace);
    }
    // --- 7. Find last sub-block and last scan position ---
    let last_scan_p = get_scan_position(last_sig_x, last_sig_y, scan_idx, log2_trafo_size);

    let last_scan_pos = last_scan_p.scan_pos;
    let last_sub_block = last_scan_p.sub_block;

    // sb_width is the number of 4x4 sub-blocks across the TU
    let sb_width = 1_usize << (log2_trafo_size - 2);

    // coded_sub_block_neighbors tracks which 4x4 groups have coefficients
    // Max TU is 32x32, so max sb_width is 8. 8*8 = 64.
    let mut coded_sub_block_neighbors = vec![0u8; sb_width * sb_width];

    // --- 8. Initialize loop state variables ---
    let mut c1 = 1i32;
    let mut first_subblock = true;
    let mut last_sub_block_gtr1_ctx = 0i32;

    debug_more!(
        "last_sub_block: {}, last_scan_pos: {}, sb_width: {}",
        last_sub_block,
        last_scan_pos,
        sb_width
    );

    let mut g1_state = Greater1State {
        bit:          0,
        greater1_ctx: 0,
        ctx_set:      0
    };

    let c_idx = match component {
        Component::Luma => 0,
        Component::Cb => 1,
        Component::Cr => 2
    };

    ctx.n_coeff[c_idx] = 0;

    // Iterate through sub-blocks in reverse scan order
    for i in (0..=last_sub_block).rev() {
        let s = scan_order_sub[i as usize];
        let mut infer_sb_dc_sig_coeff_flag = false;

        debug_more!("sub block scan idx: {}", i);

        // --- 1. Check whether this sub-block is coded (CSBF) ---
        let mut sub_block_is_coded = false;

        if i < last_sub_block && i > 0 {
            sub_block_is_coded = decode_coded_sub_block_flag(
                ctx,
                component,
                coded_sub_block_neighbors[(s.x as usize) + ((s.y as usize) * sb_width)]
            );
            infer_sb_dc_sig_coeff_flag = true;
        } else if i == 0 || i == last_sub_block {
            // first (DC) and last sub-block are always coded
            // - the first will most probably contain coefficients
            // - the last obviously contains the last coded coefficient

            sub_block_is_coded = true;
        }
        if sub_block_is_coded {
            // Mark left neighbor: it now knows its right neighbor is coded
            if s.x > 0 {
                coded_sub_block_neighbors[(s.x - 1) as usize + s.y as usize * sb_width] |= 1;
            }
            // Mark top neighbor: it now knows its bottom neighbor is coded
            if s.y > 0 {
                coded_sub_block_neighbors[s.x as usize + (s.y - 1) as usize * sb_width] |= 2;
            }
        }
        // --- find significant coefficients in this sub-block ----

        // Local arrays for this sub-block's found coefficients (max 16)
        let mut coeff_value = [0i16; 16];
        let mut coeff_scan_pos = [0i8; 16];
        let mut coeff_sign = [0u8; 16];

        let mut coeff_has_max_base_level = [0i8; 16];
        let mut n_coefficients = 0;

        if sub_block_is_coded {
            let x0 = s.x << 2;
            let y0 = s.y << 2;

            let log2w = log2_trafo_size - 2;
            let prev_csbf =
                coded_sub_block_neighbors[((s.x as usize) + ((s.y as usize) * sb_width)) as usize];

            let size_idx = (log2_trafo_size - 2) as usize;
            let chroma_idx = usize::from(component != Component::Luma);
            let scan_type_idx = scan_idx as usize;
            let csbf_idx = prev_csbf as usize;

            debug_more!(
                "log2w:{} c_idx:{}, scan_idx:{} prev_csbf: {}",
                log2w,
                chroma_idx,
                scan_type_idx,
                prev_csbf
            );
            // set the last coded coefficient in the last subblock

            let last_coeff = if i == last_sub_block { last_scan_pos - 1 } else { 15 };

            if i == last_sub_block {
                coeff_value[n_coefficients] = 1;
                coeff_has_max_base_level[n_coefficients] = 1;
                coeff_scan_pos[n_coefficients] = last_scan_pos as i8;
                n_coefficients += 1;
            }

            let rext_ts_ctx = ctx
                .sps
                .range_extension
                .as_ref()
                .is_some_and(|re| re.transform_skip_context_enabled_flag);

            // decode all coefficients significant_coeff
            // --- Pass 1: Decode significant_coeff_flags (AC coefficients) ---
            for n in (1..=last_coeff).rev() {
                let sub_x = scan_order_pos[n as usize].x;
                let sub_y = scan_order_pos[n as usize].y;
                let xc = x0 + sub_x;
                let yc = y0 + sub_y;

                // Handle Range Extension (RExt) specialized context
                let ctx_inc: usize;

                if rext_ts_ctx
                    && (ctx.cu_transquant_bypass_flag || ctx.transform_skip_flag[c_idx] > 0)
                {
                    ctx_inc = if component == Component::Luma { 42 } else { 16 + 27 };
                } else {
                    // can't propagate this due to rust errors.
                    // immutable borrow
                    let ctx_idx_map =
                        &ctx.sig_ctx_maps[size_idx][chroma_idx][scan_type_idx][csbf_idx];

                    let idx = xc as usize + ((yc as usize) << (log2_trafo_size as usize));
                    // Standard context lookup using the precomputed map
                    ctx_inc = ctx_idx_map[idx] as usize;
                }

                debug_more!("trafoSize: {}", 1 << log2_trafo_size);

                let significant_coeff = decode_significant_coeff_flag_lookup(ctx, ctx_inc);

                if significant_coeff {
                    coeff_value[n_coefficients] = 1;
                    coeff_has_max_base_level[n_coefficients] = 1;
                    coeff_scan_pos[n_coefficients] = n as i8;
                    n_coefficients += 1;

                    // If an AC coefficient exists, the DC coefficient cannot be inferred
                    infer_sb_dc_sig_coeff_flag = false;
                }
            }
            // --decode DC coeff significance ---
            if last_coeff >= 0 {
                if infer_sb_dc_sig_coeff_flag {
                    coeff_value[n_coefficients] = 1;
                    coeff_has_max_base_level[n_coefficients] = 1;
                    coeff_scan_pos[n_coefficients] = 0;
                    n_coefficients += 1;
                } else {
                    // if inference failed, its coded

                    let ctx_inc: usize;

                    if rext_ts_ctx
                        && (ctx.cu_transquant_bypass_flag || ctx.transform_skip_flag[c_idx] > 0)
                    {
                        ctx_inc = if component == Component::Luma { 42 } else { 16 + 27 };
                    } else {
                        // can't propagate this due to rust errors.
                        // immutable borrow
                        let ctx_idx_map =
                            &ctx.sig_ctx_maps[size_idx][chroma_idx][scan_type_idx][csbf_idx];

                        let idx = x0 as usize + ((y0 as usize) << (log2_trafo_size as usize));
                        // Standard context lookup using the precomputed map
                        ctx_inc = ctx_idx_map[idx] as usize;
                    }

                    let significant_coeff_flag = decode_significant_coeff_flag_lookup(ctx, ctx_inc);

                    if significant_coeff_flag {
                        coeff_value[n_coefficients] = 1;
                        coeff_has_max_base_level[n_coefficients] = 1;
                        coeff_scan_pos[n_coefficients] = 0;
                        n_coefficients += 1;
                    }
                }
            }
        }
        if n_coefficients > 0 {
            let mut ctx_set;

            if i == 0 || component != Component::Luma {
                ctx_set = 0;
            } else {
                ctx_set = 2;
            }

            if c1 == 0 {
                ctx_set += 1;
            }
            c1 = 1;

            // decode greater-1 flags
            let mut new_last_greater_1_scan_pos: i32 = -1;
            let last_greater_1_coeff = n_coefficients.min(8);

            for c in 0..last_greater_1_coeff {
                g1_state = decode_coeff_abs_level_greater1(
                    ctx,
                    c_idx,
                    i,
                    c == 0,
                    first_subblock,
                    last_sub_block_gtr1_ctx,
                    g1_state,
                    ctx_set
                );
                if g1_state.bit == 1 {
                    coeff_value[c] += 1;

                    c1 = 0;
                    if new_last_greater_1_scan_pos == -1 {
                        new_last_greater_1_scan_pos = c as i32;
                    }
                } else {
                    coeff_has_max_base_level[c] = 0;
                    if c1 < 3 && c1 > 0 {
                        c1 += 1;
                    }
                }
            }
            first_subblock = false;
            last_sub_block_gtr1_ctx = g1_state.greater1_ctx;

            // ------ decode greater-2 flag ----
            if new_last_greater_1_scan_pos != -1 {
                let flag = decode_coeff_abss_level_greater2(ctx, c_idx, g1_state.ctx_set as usize);

                coeff_value[new_last_greater_1_scan_pos as usize] += i16::from(flag);
                coeff_has_max_base_level[new_last_greater_1_scan_pos as usize] = flag as i8;
            }
            // --decode coefficient signs ---
            let sign_hidden;

            let pred_mode_intra;
            if c_idx == 0 {
                pred_mode_intra = ctx.neighbor_tracker.get_intra_mode(x0, y0);
            } else {
                pred_mode_intra = ctx.neighbor_tracker.get_intra_mode_chroma(x0, y0);
            }
            let implicit_rdpcm_enabled_flag = ctx
                .sps
                .range_extension
                .as_ref()
                .is_some_and(|s| s.implicit_rdpcm_enabled_flag);

            if ctx.cu_transquant_bypass_flag
                || (pred_mode == PredMode::ModeIntra
                    && implicit_rdpcm_enabled_flag
                    && ctx.transform_skip_flag[c_idx] == 1
                    && (pred_mode_intra == 10 || pred_mode_intra == 26))
                || ctx.explicit_rdpcm_flag
            {
                sign_hidden = false;
            } else {
                sign_hidden = coeff_scan_pos[0] - coeff_scan_pos[n_coefficients - 1] > 3;
            }

            for n in 0..n_coefficients - 1 {
                coeff_sign[n] = ctx.cabac.decode_bypass();
                debug_more!(" sign[{n}]= {}", coeff_sign[n]);
            }
            if !pps.sign_data_hiding_enabled_flag || !sign_hidden {
                coeff_sign[n_coefficients - 1] = ctx.cabac.decode_bypass();
                debug_more!(
                    " sign[{}]= {}",
                    n_coefficients - 1,
                    coeff_sign[n_coefficients - 1]
                );
            } else {
                coeff_sign[n_coefficients - 1] = 0;
            }

            // --- decode coefficient value --
            let mut sum_abs_level: i32 = 0;
            let mut ui_go_rice_param: u8;

            // sb_type logic (usually 0 for Luma, 1 for Chroma in standard, more in RExt)
            let sb_type = usize::from(c_idx != 0);
            let persistent_rice_adaptation_enabled_flag = ctx
                .sps
                .range_extension
                .as_ref()
                .is_some_and(|s| s.persistent_rice_adaptation_enabled_flag);

            if persistent_rice_adaptation_enabled_flag {
                ui_go_rice_param = ctx.stat_coeff[sb_type] / 4;
            } else {
                ui_go_rice_param = 0;
            }

            let mut first_coeff_with_abs_level_remaining = true;

            for n in 0..n_coefficients {
                let base_level = i32::from(coeff_value[n]);
                let mut coeff_abs_level_remaining = 0;

                if coeff_has_max_base_level[n] != 0 {
                    coeff_abs_level_remaining =
                        decode_coeff_abs_level_remaining(ctx, ui_go_rice_param);

                    // Update Rice Parameter (uiGoRiceParam)
                    let total_abs = base_level + coeff_abs_level_remaining;
                    let threshold = 3 << ui_go_rice_param;

                    if total_abs > threshold {
                        ui_go_rice_param = (ui_go_rice_param + 1).min(4);
                    }

                    // Persistent Rice Adaptation (HEVC RExt)
                    if persistent_rice_adaptation_enabled_flag
                        && first_coeff_with_abs_level_remaining
                    {
                        let stat_val = ctx.stat_coeff[sb_type];
                        let shift = stat_val / 4;

                        if coeff_abs_level_remaining >= (3 << shift) {
                            ctx.stat_coeff[sb_type] += 1;
                        } else if (2 * coeff_abs_level_remaining) < (1 << shift) && stat_val > 0 {
                            ctx.stat_coeff[sb_type] -= 1;
                        }
                        first_coeff_with_abs_level_remaining = false;
                    }
                }

                debug_more!("coeff_abs_level_remaining={}", coeff_abs_level_remaining);

                // Reconstruct Magnitude
                let mut curr_coeff = base_level + coeff_abs_level_remaining;

                // Apply Sign
                if coeff_sign[n] != 0 {
                    curr_coeff = -curr_coeff;
                }

                // Sign Data Hiding (SDH)
                // In HEVC, the sign of the first coefficient (in scan order) can be hidden
                if ctx.pps.sign_data_hiding_enabled_flag && sign_hidden {
                    sum_abs_level += curr_coeff;

                    // If parity doesn't match, flip the sign of the last coefficient in the list
                    if n == n_coefficients - 1 && (sum_abs_level & 1) != 0 {
                        curr_coeff = -curr_coeff;
                    }
                }

                debug_more!("quantized coefficient={}", curr_coeff);

                // Map back to 2D Raster Coordinates
                let p = coeff_scan_pos[n] as usize;
                let xc = (u32::from(s.x) << 2) + u32::from(scan_order_pos[p].x);
                let yc = (u32::from(s.y) << 2) + u32::from(scan_order_pos[p].y);

                let coeff_stride = 1u32 << log2_trafo_size;

                // Store in Final List
                let final_val = curr_coeff.clamp(-32768, 32767) as i16;
                let pos = (xc + yc * coeff_stride) as usize;

                ctx.coeff_list[c_idx][ctx.n_coeff[c_idx] as usize] = final_val;
                ctx.coeff_pos[c_idx][ctx.n_coeff[c_idx] as usize] = pos as i16;
                ctx.n_coeff[c_idx] += 1;
            }
        }
    }
}
