use std::sync::Arc;

use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::ctx::DecodeSliceContext;

// ============================================================
// Caller contract (document these or add a wrapper that checks):
//   p.len()    >= 4 * n_t + 1
//   dst.len()  >= n_t * n_t
// ============================================================

#[inline(always)]
fn check_predict_args(p: &[u8], dst: &[u8], n_t: usize) {
    assert!(
        p.len() >= 4 * n_t + 1,
        "p too short: need {}, got {}",
        4 * n_t + 1,
        p.len()
    );
    assert!(
        dst.len() >= n_t * n_t,
        "dst too short: need {}, got {}",
        n_t * n_t,
        dst.len()
    );
}

// ── predict_dc ────────────────────────────────────────────────

pub fn predict_dc(p: &[u8], dst: &mut [u8], n_t: usize, log2_n_t: u8, is_luma: bool) {
    check_predict_args(p, dst, n_t);

    // Sum the top row (p[1..=n_t]) and left column (p[1+2*n_t..=1+3*n_t])
    // in a single pass by chaining the two slices.
    let sum: u32 = p[1..=n_t]
        .iter()
        .chain(&p[1 + 2 * n_t..=1 + 3 * n_t])
        .fold(0u32, |acc, &v| acc + v as u32);

    let dc_val = ((sum + n_t as u32) >> (log2_n_t + 1)) as u8;
    dst.fill(dc_val);

    // DC filter (luma, block sizes < 32 only — Spec 8.4.4.2.5)
    if is_luma && n_t < 32 {
        let dc_i32 = dc_val as i32;

        // Top-left corner
        dst[0] = ((p[1] as i32 + 2 * dc_i32 + p[1 + 2 * n_t] as i32 + 2) >> 2) as u8;

        // Top row (x = 1..n_t)  — zip avoids recomputing the index
        dst[1..n_t]
            .iter_mut()
            .zip(&p[2..=n_t])
            .for_each(|(d, &above)| {
                *d = ((above as i32 + 3 * dc_i32 + 2) >> 2) as u8;
            });

        // Left column (y = 1..n_t)
        let left = &p[2 + 2 * n_t..=3 * n_t]; // p[1+2*n_t+1 .. 1+2*n_t+n_t-1]
        dst[n_t..]
            .iter_mut()
            .step_by(n_t)
            .zip(left)
            .for_each(|(d, &lv)| {
                *d = ((lv as i32 + 3 * dc_i32 + 2) >> 2) as u8;
            });
    }
}

// ── predict_planar ────────────────────────────────────────────

pub fn predict_planar(p: &[u8], dst: &mut [u8], n_t: usize, log2_n_t: u8) {
    check_predict_args(p, dst, n_t);

    let top_right = p[n_t + 1] as i32;
    let bottom_left = p[3 * n_t + 1] as i32;
    let shift = log2_n_t + 1;
    let n = n_t as i32;
    let dst = &mut dst[..n_t * n_t];

    dst.chunks_exact_mut(n_t).enumerate().for_each(|(y, row)| {
        let y = y as i32;
        let idx = 1 + 2 * n_t + y as usize;
        let left_val = p[idx] as i32;
        let v_weight_b = y + 1; // (y+1) * bottom_left
        let v_weight_t = n - 1 - y; // (n-1-y) * top_val

        row.iter_mut().enumerate().for_each(|(x, dst_px)| {
            let x = x as i32;
            let top_val = p[1 + x as usize] as i32;
            // Horizontal linear interpolation
            let h = (n - 1 - x) * left_val + (x + 1) * top_right;
            // Vertical linear interpolation
            let v = v_weight_t * top_val + v_weight_b * bottom_left;

            *dst_px = ((h + v + n) >> shift) as u8;
        });
    });
}

// ── predict_angular ───────────────────────────────────────────
//
// Caller must also provide:
//   ref_main_buf: &mut [u8]  with len >= 3 * n_t + 1
//
// Using a caller-supplied buffer eliminates the per-call Vec allocation
// entirely while keeping the function free of unsafe code.

// HEVC intraPredAngle table (Spec Table 8-2)
#[rustfmt::skip]
const INTRA_ANGLES: [i16; 35] = [
      0,  0,  32, 26, 21, 17, 13,
      9,  5,   2,  0, -2, -5, -9,
    -13,-17, -21,-26,-32,-26,-21,
    -17,-13,  -9, -5, -2,  0,  2,
      5,  9,  13, 17, 21, 26, 32,
];

#[rustfmt::skip]
/// HEVC invAngle table (Table 8-4). 
/// Maps mode_idx (0..34) to the scaled inverse of the intraPredAngle.
#[rustfmt::skip]
/// HEVC invAngle table (Table 8-4).
/// Maps mode_idx (0..34) to the scaled inverse of the intraPredAngle.
/// Required for all negative-angle modes: 2-9 (Horizontal) and 18-25 (Vertical).
const INV_ANGLES: [i16; 35] = [
    0, 0,                                         // 0: Planar, 1: DC
    256, 315, 390, 482, 630, 910, 1638, 4096,     // 2-9:   Negative Horizontal
    0, 0, 0, 0, 0, 0, 0, 0,                       // 10-17: Positive Horizontal
    256, 315, 390, 482, 630, 910, 1638, 4096,     // 18-25: Negative Vertical
    0, 0, 0, 0, 0, 0, 0, 0, 0                     // 26-34: Positive Vertical
];

pub fn predict_angular(p: &[u8], dst: &mut [u8], ref_main_buf: &mut [u8], n_t: usize, mode: u8) {
    let mode_idx = mode as usize;
    let angle = INTRA_ANGLES[mode_idx];
    let is_vert = mode >= 18;
    let offset = n_t;

    // ── 1. Build ref_main ────────────────────────────────────────
    if is_vert {
        // Main: Top. Side: Left.
        ref_main_buf[offset] = p[0]; // Corner (-1,-1)
        ref_main_buf[offset + 1..=offset + 2 * n_t].copy_from_slice(&p[1..=2 * n_t]);

        if angle < 0 {
            let inv_angle = INV_ANGLES[mode_idx] as i32;
            let proj_len = ((n_t as i32 * angle as i32) >> 5).abs() as usize;
            for i in 1..=proj_len {
                let side_idx = ((i as i32 * inv_angle + 128) >> 8) as usize;
                // p[1 + 2*n_t] is the start of Left samples in your 4N+1 buffer
                ref_main_buf[offset - i] = p[1 + 2 * n_t + (side_idx - 1)];
            }
        }
    } else {
        // Main: Left. Side: Top.
        ref_main_buf[offset] = p[0]; // Corner (-1,-1)
        ref_main_buf[offset + 1..=offset + 2 * n_t].copy_from_slice(&p[1 + 2 * n_t..=4 * n_t]);

        if angle < 0 {
            let inv_angle = INV_ANGLES[mode_idx] as i32;
            let proj_len = ((n_t as i32 * angle as i32) >> 5).abs() as usize;
            for i in 1..=proj_len {
                let side_idx = ((i as i32 * inv_angle + 128) >> 8) as usize;
                // p[1] is the start of Top samples
                ref_main_buf[offset - i] = p[1 + (side_idx - 1)];
            }
        }
    }

    // ── 2. Project into dst (1/32-pixel interpolation) ───────────
    for y in 0..n_t {
        // The spec uses distance (y + 1) from the reference line
        let pos = ((y + 1) as i32) * (angle as i32);
        let int_pos = (pos >> 5);
        let frac = (pos & 31) as u32;

        for x in 0..n_t {
            // THE FIX: Adding +1 to the base index.
            // This aligns ref_main_buf[offset + 1] with the first Top/Left sample.
            let base = (offset as i32 + x as i32 + int_pos + 1) as usize;

            let val = if frac != 0 {
                let s1 = ref_main_buf[base] as u32;
                let s2 = ref_main_buf[base + 1] as u32;
                (((32 - frac) * s1 + frac * s2 + 16) >> 5) as u8
            } else {
                ref_main_buf[base]
            };

            let (row, col) = if is_vert { (y, x) } else { (x, y) };
            dst[row * n_t + col] = val;
        }
    }
}
pub fn decode_intra_prediction_internal_u8(
    ctx: &mut DecodeSliceContext, x_b0: usize, y_b0: usize, intra_mode: u8, n_t: usize,
    c_idx: usize
) {
    // 1. Setup Reference Samples
    // We need (2 * nT + 1) samples for both the Top and Left arrays.
    // These are pulled from already reconstructed pixels in the current frame.

    let p_len = ctx.setup_reference_samples(x_b0, y_b0, n_t, intra_mode, c_idx); // 2. Dispatch to the specific Mode Logic
    let scratchpad = &mut ctx.pixel_scratchpad;
    let ref_main_scratch = &mut ctx.ref_main_buf;

    let p_slice = &ctx.ref_samples_p[..p_len];
    let log2_n_t = n_t.trailing_zeros() as u8;

    let bit_depth = if c_idx == 0 { ctx.sps.bit_depth_luma } else { ctx.sps.bit_depth_chroma };

    if DEBUG_MORE {
        println!(
            "--- Intra Prediction Trace: Mode {}, Size {}x{}, Comp {} at [{},{}] ---",
            intra_mode, n_t, n_t, c_idx, x_b0, y_b0
        );
    }
    match intra_mode {
        0 => predict_planar(p_slice, scratchpad, n_t, log2_n_t),
        1 => predict_dc(p_slice, scratchpad, n_t, log2_n_t, c_idx == 0),
        2..=34 => predict_angular(&p_slice, scratchpad, ref_main_scratch, n_t, intra_mode),
        _ => unreachable!()
    }

    if DEBUG_MORE {
        // --- DEBUG PRINT SECTION ---
        if n_t <= 32 {
            println!(
                "--- Intra Prediction Trace: Mode {}, Size {}x{}, Comp {} at [{},{}] ---",
                intra_mode, n_t, n_t, c_idx, x_b0, y_b0
            );
            for y in 0..n_t {
                for x in 0..n_t {
                    print!("{:3} ", scratchpad[y * n_t + x]);
                }
                println!();
            }
            println!("------------------------------------------------------------");
        }
    }
    ctx.write_block_scratchpad(c_idx, x_b0, y_b0, n_t, bit_depth);
}
pub fn decode_intra_prediction(
    ctx: &mut DecodeSliceContext,
    x_b0: usize,
    y_b0: usize,
    intra_pred_mode: u8,
    n_t: usize, // TU size (4, 8, 16, 32)
    c_idx: usize
) {
    debug_more!(
        "decode_intra_prediction xy0:{}/{} mode={} nT={}, cIdx={}",
        x_b0,
        y_b0,
        intra_pred_mode,
        n_t,
        c_idx
    );

    // For now, we assume 8-bit as requested.

    // We pass the coordinates and the component index so the internal function
    // can grab the correct pixels from the reconstructed frame buffer.
    decode_intra_prediction_internal_u8(ctx, x_b0, y_b0, intra_pred_mode, n_t, c_idx);
}
