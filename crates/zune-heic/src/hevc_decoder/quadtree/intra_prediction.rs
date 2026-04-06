
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
        p.len() > 4 * n_t,
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

    // Top row (x = 0..n_t-1) -> indices 2N+1 to 3N
    let top_slice = &p[2 * n_t + 1..=3 * n_t];

    // Left column (y = 0..n_t-1) -> indices N to 2N-1
    let left_slice = &p[n_t..(2 * n_t)];

    // Sum the top row and left column in a single pass
    let sum: u32 = top_slice
        .iter()
        .chain(left_slice)
        .fold(0u32, |acc, &v| acc + u32::from(v));

    let dc_val = ((sum + n_t as u32) >> (log2_n_t + 1)) as u8;
    dst.fill(dc_val);

    // DC filter (luma, block sizes < 32 only — Spec 8.4.4.2.5)
    if is_luma && n_t < 32 {
        let dc_i32 = i32::from(dc_val);

        // Top-left corner (y=0, x=0)
        // Top(x=0) is index 2*n_t + 1. Left(y=0) is index 2*n_t - 1.
        dst[0] = ((i32::from(p[2 * n_t + 1]) + 2 * dc_i32 + i32::from(p[2 * n_t - 1]) + 2) >> 2) as u8;

        // Top row (x = 1..n_t-1)
        dst[1..n_t]
            .iter_mut()
            .zip(&p[2 * n_t + 2..=3 * n_t])
            .for_each(|(d, &above)| {
                *d = ((i32::from(above) + 3 * dc_i32 + 2) >> 2) as u8;
            });

        // Left column (y = 1..n_t-1)
        // Left pixels for y=1..n_t-1 are indices 2*n_t - 2 down to n_t.
        let left_y1_to_n = &p[n_t..=2 * n_t - 2];

        dst[n_t..]
            .iter_mut()
            .step_by(n_t)
            // We use .rev() because the slice goes from y=N-1 up to y=1,
            // but the dst step_by goes from y=1 down to y=N-1!
            .zip(left_y1_to_n.iter().rev())
            .for_each(|(d, &lv)| {
                *d = ((i32::from(lv) + 3 * dc_i32 + 2) >> 2) as u8;
            });
    }
}
// ── predict_planar ────────────────────────────────────────────

pub fn predict_planar(p: &[u8], dst: &mut [u8], n_t: usize, log2_n_t: u8) {
    check_predict_args(p, dst, n_t);

    // top_right: x = n_t -> index = 2*n_t + 1 + n_t = 3*n_t + 1
    let top_right = i32::from(p[3 * n_t + 1]);

    // bottom_left: y = n_t -> index = 2*n_t - 1 - n_t = n_t - 1
    let bottom_left = i32::from(p[n_t - 1]);

    let shift = log2_n_t + 1;
    let n = n_t as i32;
    let dst = &mut dst[..n_t * n_t];

    dst.chunks_exact_mut(n_t).enumerate().for_each(|(y, row)| {
        let y_i32 = y as i32;

        // left_val: at current y -> index = 2*n_t - 1 - y
        let left_val = i32::from(p[2 * n_t - 1 - y]);

        let v_weight_b = y_i32 + 1; // (y+1) * bottom_left
        let v_weight_t = n - 1 - y_i32; // (n-1-y) * top_val

        row.iter_mut().enumerate().for_each(|(x, dst_px)| {
            let x_i32 = x as i32;

            // top_val: at current x -> index = 2*n_t + 1 + x
            let top_val = i32::from(p[2 * n_t + 1 + x]);

            // Horizontal linear interpolation
            let h = (n - 1 - x_i32) * left_val + (x_i32 + 1) * top_right;
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

    // Corner is exactly at index 2*n_t in the new linear layout
    let corner_idx = 2 * n_t;

    // ── 1. Build ref_main ────────────────────────────────────────
    if is_vert {
        // Main: Top. Side: Left.
        ref_main_buf[offset] = p[corner_idx];

        // Top pixels are from (2*n_t + 1) to (4*n_t)
        ref_main_buf[offset + 1..=offset + 2 * n_t]
            .copy_from_slice(&p[corner_idx + 1..=4 * n_t]);

        if angle < 0 {
            let inv_angle = i32::from(INV_ANGLES[mode_idx]);
            let proj_len = ((n_t as i32 * i32::from(angle)) >> 5).unsigned_abs() as usize;

            for i in 1..=proj_len {
                let side_idx = ((i as i32 * inv_angle + 128) >> 8) as usize;
                // Clamp side_idx so we don't read past the available 2N side pixels
                let safe_side = side_idx.min(2 * n_t).max(1);

                // Left pixels go downwards from (2*n_t - 1) to 0.
                // Distance 1 from corner is 2*n_t - 1. Distance side_idx is 2*n_t - side_idx.
                ref_main_buf[offset - i] = p[corner_idx - safe_side];
            }
        }
    } else {
        // Main: Left. Side: Top.
        ref_main_buf[offset] = p[corner_idx];

        // Left pixels are backwards in `p` (Bottom-to-Top), so we must reverse them into ref_main
        for y in 1..=2 * n_t {
            ref_main_buf[offset + y] = p[corner_idx - y];
        }

        if angle < 0 {
            let inv_angle = i32::from(INV_ANGLES[mode_idx]);
            let proj_len = ((n_t as i32 * i32::from(angle)) >> 5).unsigned_abs() as usize;

            for i in 1..=proj_len {
                let side_idx = ((i as i32 * inv_angle + 128) >> 8) as usize;
                // Clamp side_idx
                let safe_side = side_idx.min(2 * n_t).max(1);

                // Top pixels go rightwards from (2*n_t + 1) to 4*n_t.
                ref_main_buf[offset - i] = p[corner_idx + safe_side];
            }
        }
    }

    // ── 2. Project into dst (1/32-pixel interpolation) ───────────
    for y in 0..n_t {
        let pos = ((y + 1) as i32) * i32::from(angle);
        let int_pos = pos >> 5;
        let frac = (pos & 31) as u32;

        for x in 0..n_t {
            let base = (offset as i32 + x as i32 + int_pos + 1) as usize;

            let val = if frac != 0 {
                let s1 = u32::from(ref_main_buf[base]);
                let s2 = u32::from(ref_main_buf[base + 1]);
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
    // These are pulled from already reconstructed pixels in the current frame.
    let p_len = ctx.setup_reference_samples(x_b0, y_b0, n_t, intra_mode, c_idx); // 2. Dispatch to the specific Mode Logic
    let scratchpad = &mut ctx.pixel_scratchpad;
    let ref_main_scratch = &mut ctx.ref_main_buf;

    let p_slice = &ctx.ref_samples_p[..p_len];
    let log2_n_t = n_t.trailing_zeros() as u8;
    
    if DEBUG_MORE.load(std::sync::atomic::Ordering::Relaxed)  {
        println!(
            "--- Intra Prediction Trace: Mode {intra_mode}, Size {n_t}x{n_t}, Comp {c_idx} at [{x_b0},{y_b0}] ---"
        );
    }
    match intra_mode {
        0 => predict_planar(p_slice, scratchpad, n_t, log2_n_t),
        1 => predict_dc(p_slice, scratchpad, n_t, log2_n_t, c_idx == 0),
        2..=34 => predict_angular(p_slice, scratchpad, ref_main_scratch, n_t, intra_mode),
        _ => unreachable!()
    }

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
