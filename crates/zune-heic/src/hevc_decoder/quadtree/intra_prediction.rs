use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::quadtree::DecodeSliceContext;

/// The "Wall of Pixels" used for intra prediction.
/// Size is 4 * n_t + 1.
pub struct IntraReferenceSamples {
    pub p:   Vec<u8>,
    pub n_t: usize
}

impl IntraReferenceSamples {
    /// Main Entry Point: Logic to fetch, pad, and smooth reference samples.
    pub fn setup(
        ctx: &DecodeSliceContext, x0: usize, y0: usize, n_t: usize, intra_mode: u8, c_idx: usize
    ) -> Self {
        let mut p = vec![0u8; 4 * n_t + 1];
        let mut available = vec![false; 4 * n_t + 1];

        // 1. Check Availability
        check_availability(ctx, x0, y0, n_t, &mut available);

        // 2. Fetch and Pad
        perform_padding(ctx, &mut p, &available, x0, y0, n_t, c_idx);

        // 3. Filter/Smoothing
        let strong_enabled = ctx.sps.strong_intra_smoothing_enable_flag;
        apply_reference_smoothing(&mut p, n_t, intra_mode, strong_enabled);

        IntraReferenceSamples { p, n_t }
    }
}

/// Determines which neighbor pixels are allowed to be used.
fn check_availability(
    ctx: &DecodeSliceContext, x0: usize, y0: usize, n_t: usize, available: &mut [bool]
) {
    // 0: Top-Left
    available[0] = ctx
        .neighbor_tracker
        .is_available(x0, y0, x0 as isize - 1, y0 as isize - 1);

    // 1..2nT: Top and Top-Right
    for i in 0..(2 * n_t) {
        available[1 + i] =
            ctx.neighbor_tracker
                .is_available(x0, y0, (x0 + i) as isize, y0 as isize - 1);
    }

    // 2nT+1..4nT: Left and Below-Left
    for i in 0..(2 * n_t) {
        available[1 + 2 * n_t + i] =
            ctx.neighbor_tracker
                .is_available(x0, y0, x0 as isize - 1, (y0 + i) as isize);
    }

    // Optional: Constraint check for error resilience
    if ctx.pps.constrained_intra_pred_flag {
        filter_constrained(ctx, x0, y0, n_t, available);
    }
}

/// Pulls pixels from the Mutex-protected RawFrame and spreads them to gaps.
fn perform_padding(
    ctx: &DecodeSliceContext, p: &mut [u8], available: &[bool], x0: usize, y0: usize, n_t: usize,
    c_idx: usize
) {
    // 1. Lock the plane once to fetch available pixels
    let plane = match c_idx {
        0 => ctx.raw_frame.luma.lock().unwrap(),
        1 => ctx.raw_frame.cb.lock().unwrap(),
        2 => ctx.raw_frame.cr.lock().unwrap(),
        _ => unreachable!()
    };

    let get_pixel = |px: isize, py: isize| -> u8 {
        let stride = plane.stride;
        let padding = plane.padding;
        let idx = (py as usize + padding) * stride + (px as usize + padding);
        plane.pixels[idx]
    };

    if available[0] {
        p[0] = get_pixel(x0 as isize - 1, y0 as isize - 1);
    }
    for i in 0..(2 * n_t) {
        if available[1 + i] {
            p[1 + i] = get_pixel((x0 + i) as isize, y0 as isize - 1);
        }
        if available[1 + 2 * n_t + i] {
            p[1 + 2 * n_t + i] = get_pixel(x0 as isize - 1, (y0 + i) as isize);
        }
    }

    drop(plane); // Release lock before math

    // 2. Define Search Order (Bottom-Left -> Up -> Across to Top-Right)
    let mut order = Vec::with_capacity(4 * n_t + 1);
    for i in ((2 * n_t + 1)..=(4 * n_t)).rev() {
        order.push(i);
    }
    order.push(0);
    for i in 1..=(2 * n_t) {
        order.push(i);
    }

    // 3. First-pass: If nothing is available, fill with 128
    if !available.iter().any(|&a| a) {
        p.fill(128);
        return;
    }

    // 4. Second-pass: Propagation
    let first_valid_idx = *order.iter().find(|&&idx| available[idx]).unwrap();
    let mut last_val = p[first_valid_idx];

    for &idx in &order {
        if available[idx] {
            last_val = p[idx];
        } else {
            p[idx] = last_val;
        }
    }
}

/// Applies [1, 2, 1] smoothing or Strong Intra Smoothing (Spec 8.4.4.2.3)
fn apply_reference_smoothing(p: &mut [u8], n_t: usize, mode: u8, strong_enabled: bool) {
    if n_t == 4 {
        return;
    } // 4x4 is never smoothed

    // Strong Intra Smoothing check for 32x32 blocks
    if n_t == 32 && strong_enabled {
        let threshold = 1 << (8 - 5); // Default for 8-bit
        let tl = p[0] as i32;
        let tr = p[2 * n_t] as i32;
        let bl = p[4 * n_t] as i32;

        if (tl + tr - 2 * p[n_t] as i32).abs() < threshold
            && (tl + bl - 2 * p[3 * n_t] as i32).abs() < threshold
        {
            apply_strong_smoothing(p, n_t);
            return;
        }
    }

    // Standard [1, 2, 1] smoothing
    if is_filtering_required(mode, n_t) {
        let mut p_copy = p.to_vec();
        for i in 1..4 * n_t {
            p_copy[i] = ((p[i - 1] as u16 + 2 * p[i] as u16 + p[i + 1] as u16 + 2) >> 2) as u8;
        }
        // Corner and ends are not smoothed or use specific rules;
        // standard HEVC logic skips p[0] and p[4*nT] during standard filter.
        p.copy_from_slice(&p_copy);
    }
}

fn apply_strong_smoothing(p: &mut [u8], n_t: usize) {
    let tl = p[0] as i32;
    let tr = p[2 * n_t] as i32;
    let bl = p[4 * n_t] as i32;

    // Top edge bilinear interpolation
    for i in 1..=2 * n_t {
        p[i] = (((2 * n_t - i) as i32 * tl + i as i32 * tr + n_t as i32) / (2 * n_t) as i32) as u8;
    }
    // Left edge bilinear interpolation
    for i in 1..=2 * n_t {
        p[2 * n_t + i] =
            (((2 * n_t - i) as i32 * tl + i as i32 * bl + n_t as i32) / (2 * n_t) as i32) as u8;
    }
}

fn is_filtering_required(mode: u8, n_t: usize) -> bool {
    // HEVC Table 8-3
    match n_t {
        8 => mode == 0 || mode == 2 || mode == 18 || mode == 34,
        16 => mode != 1 && mode != 10 && mode != 26,
        32 => mode != 1 && mode != 10 && mode != 26,
        _ => false
    }
}

fn filter_constrained(
    ctx: &DecodeSliceContext, x0: usize, y0: usize, n_t: usize, available: &mut [bool]
) {
    let mut check = |px: usize, py: usize, idx: usize| {
        if available[idx] && !ctx.neighbor_tracker.get_state(px, py).is_intra {
            available[idx] = false;
        }
    };

    check(x0 - 1, y0 - 1, 0);
    for i in 0..(2 * n_t) {
        check(x0 + i, y0 - 1, 1 + i);
        check(x0 - 1, y0 + i, 1 + 2 * n_t + i);
    }
}

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

    dst.chunks_exact_mut(n_t).enumerate().for_each(|(y, row)| {
        let y = y as i32;
        let left_val = p[1 + 2 * n_t + y as usize] as i32;
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
const INV_ANGLES: [i16; 35] = [
     0,    0,    0,    0,    0,    0,    0,    0,    0,    0,    0,
 -4096, -1638,  -910,  -630,  -482,  -390,  -315,  -256,  -315,  -390,  -482,  -630,  -910, -1638, -4096,
     0,    0,    0,    0,    0,    0,    0,    0,    0,
];

pub fn predict_angular(
    p: &[u8],
    dst: &mut [u8],
    ref_main_buf: &mut [u8], // must be >= 3*n_t + 1; caller allocates once
    n_t: usize,
    mode: u8
) {
    check_predict_args(p, dst, n_t);
    assert!(
        ref_main_buf.len() >= 3 * n_t + 1,
        "ref_main_buf too short: need {}, got {}",
        3 * n_t + 1,
        ref_main_buf.len()
    );

    let mode_idx = mode as usize;
    let angle = INTRA_ANGLES[mode_idx];
    let is_vert = mode >= 18;

    // We keep a virtual index `offset` = n_t so that ref_main[offset + i]
    // corresponds to the spec's ref_main[i], and ref_main[offset - i]
    // handles the negative projections without ever forming a negative usize.
    let offset = n_t;

    // ── build ref_main ────────────────────────────────────────
    if is_vert {
        // Vertical-like modes 18-34: seed from top/corner row
        ref_main_buf[offset..=offset + n_t]
            .iter_mut()
            .zip(&p[0..=n_t])
            .for_each(|(r, &s)| *r = s);

        if angle < 0 {
            let inv_angle = INV_ANGLES[mode_idx];
            let proj_len = (((n_t as i32) * (angle as i32)) >> 5).unsigned_abs() as usize;

            for i in 1..=proj_len {
                let left_idx = (((i as i32) * (inv_angle as i32) + 128) >> 8) as usize;
                ref_main_buf[offset - i] = p[1 + 2 * n_t + left_idx.saturating_sub(1)];
            }
        } else {
            ref_main_buf[offset + n_t + 1..=offset + 2 * n_t]
                .iter_mut()
                .zip(&p[n_t + 1..=2 * n_t])
                .for_each(|(r, &s)| *r = s);
        }
    } else {
        // Horizontal-like modes 2-17: seed from left/corner column
        ref_main_buf[offset..=offset + n_t]
            .iter_mut()
            .zip(&p[2 * n_t..=3 * n_t])
            .for_each(|(r, &s)| *r = s);

        if angle < 0 {
            let inv_angle = INV_ANGLES[mode_idx];
            let proj_len = (((n_t as i32) * (angle as i32)) >> 5).unsigned_abs() as usize;

            for i in 1..=proj_len {
                let above_idx = (((i as i32) * (inv_angle as i32) + 128) >> 8) as usize;
                ref_main_buf[offset - i] = p[above_idx];
            }
        } else {
            ref_main_buf[offset + n_t + 1..=offset + 2 * n_t]
                .iter_mut()
                .zip(&p[3 * n_t + 1..=4 * n_t])
                .for_each(|(r, &s)| *r = s);
        }
    }

    // ── project into dst (1/32-pixel interpolation) ───────────
    //
    // Unified: swapping (row, col) vs (col, row) on the dst write
    // replaces the duplicated if/else inside the hot x-loop.
    for y in 0..n_t {
        let pos = ((y + 1) as i32) * (angle as i32);
        let int_pos = (pos >> 5) as isize;
        let frac = (pos & 31) as u32;

        for x in 0..n_t {
            // ref_main virtual index: offset + x + int_pos
            let base = (offset as isize + x as isize + int_pos) as usize;

            let val = if frac != 0 {
                let s1 = ref_main_buf[base] as u32;
                let s2 = ref_main_buf[base + 1] as u32;
                (((32 - frac) * s1 + frac * s2 + 16) >> 5) as u8
            } else {
                ref_main_buf[base]
            };

            // Vertical: dst[y][x];
            // Horizontal: transposed dst[x][y]
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

    let ref_samples = IntraReferenceSamples::setup(ctx, x_b0, y_b0, n_t, intra_mode, c_idx);
    // 2. Dispatch to the specific Mode Logic

    let scratchpad = &mut ctx.scratchpad;
    let ref_main_scratch = &mut ctx.ref_main_buf;
    let log2_n_t = n_t.trailing_zeros() as u8;

    match intra_mode {
        0 => predict_planar(&ref_samples.p, scratchpad, n_t, log2_n_t),
        1 => predict_dc(&ref_samples.p, scratchpad, n_t, log2_n_t, c_idx == 0),
        2..=34 => predict_angular(
            &ref_samples.p,
            scratchpad,
            ref_main_scratch,
            n_t,
            intra_mode
        ),
        _ => unreachable!()
    }

    // --- DEBUG PRINT SECTION ---
    // Only printing for smaller blocks to avoid flooding the terminal
    if n_t <= 32 {
        println!(
            "--- Intra Prediction Trace: Mode {}, Size {}x{}, Comp {} at [{},{}] ---",
            intra_mode, n_t, n_t, c_idx, x_b0, y_b0
        );
        for y in 0..n_t {
            print!("  Row {:2}: ", y);
            for x in 0..n_t {
                print!("{:3} ", scratchpad[y * n_t + x]);
            }
            println!();
        }
        println!("------------------------------------------------------------");
    }
    ctx.write_block(c_idx, x_b0, y_b0, n_t);
    panic!();
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
