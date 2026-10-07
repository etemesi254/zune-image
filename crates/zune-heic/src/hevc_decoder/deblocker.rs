/// HEVC Deblocking Filter — ITU-T H.265 Section 8.7
///
/// Pipeline:
///   1. Compute boundary strength (BS) for every 4×4 edge grid.
///   2. For each 8×8-aligned boundary decide whether to filter and how
///      strongly (normal vs strong luma, chroma on/off).
///   3. Apply in two passes: vertical edges (left boundary of each column)
///      then horizontal edges (top boundary of each row).
///
/// Key spec tables reproduced inline:
///   • Table 8-19  QP → β index
///   • Table 8-20  QP → tC index  (indexed as QP + 2*(bs-1))
use std::sync::MutexGuard;

use crate::hevc_decoder::neighbor_tracker::{BlockState, NeighborTracker};
use crate::hevc_decoder::raw_frame::{RawFrame, SingleFrame, offset_plane};
/// β table — indexed by Clip3(0,51, qP).  spec Table 8-19.
#[rustfmt::skip]
const BETA_TABLE: [u8; 52] = [
     0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
     0,  0,  0,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15,
    16, 17, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38,
    40, 42, 44, 46, 48, 50, 52, 54, 56, 58, 60, 62, 64,
];

/// tC table — indexed by Clip3(0,53, qP + 2*(bs-1)).  spec Table 8-20.
///
///   tc(32, 1) → idx 32 → 3
///   tc(32, 2) → idx 34 → 3
///   tc(51, 2) → idx 53 → 24
#[rustfmt::skip]
const TC_TABLE: [u8; 54] = [
     0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  //  0– 9
     0,  0,  0,  0,  0,  0,  0,  0,  1,  1,  // 10–19
     1,  1,  1,  1,  1,  1,  1,  2,  2,  2,  // 20–29
     2,  3,  3,  3,  3,  4,  4,  4,  5,  5,  // 30–39
     6,  6,  7,  8,  9, 10, 11, 13, 14, 16,  // 40–49
    18, 20, 22, 24,                          // 50–53
];

/// qPi → qPc mapping.  spec Table 8-15.
///
///   qPi=30 → 29,  qPi=35 → 33,  qPi=45 → 39,  qPi=51 → 45.
#[rustfmt::skip]
const CHROMA_QP_TABLE: [i8; 52] = [
     0,  1,  2,  3,  4,  5,  6,  7,  8,  9,  //  0– 9
    10, 11, 12, 13, 14, 15, 16, 17, 18, 19,  // 10–19
    20, 21, 22, 23, 24, 25, 26, 27, 28, 29,  // 20–29
    29, 30, 31, 32, 33, 33, 34, 34, 35, 35,  // 30–39
    36, 36, 37, 37, 38, 39, 40, 41, 42, 43,  // 40–49
    44, 45,                                  // 50–51
];

#[inline]
fn beta(qp: i32) -> i32 {
    i32::from(BETA_TABLE[qp.clamp(0, 51) as usize])
}

#[inline]
fn tc(qp: i32, bs: u8) -> i32 {
    let idx = (qp + 2 * (i32::from(bs) - 1)).clamp(0, 53) as usize;
    i32::from(TC_TABLE[idx])
}

/// Slice-level deblocking parameters (spec 7.4.7.1).
#[derive(Clone, Copy, Debug, Default)]
pub struct DeblockParams {
    pub beta_offset_div2: i8,
    pub tc_offset_div2:   i8,
    pub cb_qp_offset:     i8,
    pub cr_qp_offset:     i8
}

impl DeblockParams {
    #[inline]
    fn beta(self, qp: i32) -> i32 {
        beta(qp + 2 * i32::from(self.beta_offset_div2))
    }
    #[inline]
    fn tc(self, qp: i32, bs: u8) -> i32 {
        tc(qp + 2 * i32::from(self.tc_offset_div2), bs)
    }
}

#[inline]
fn chroma_qp(luma_qp: i32, offset: i8) -> i32 {
    let qp_i = (luma_qp + i32::from(offset)).clamp(0, 51);
    i32::from(CHROMA_QP_TABLE[qp_i as usize])
}

// ---------------------------------------------------------------------------
// Boundary-strength grid
// ---------------------------------------------------------------------------

struct BsGrid {
    bs_v: Vec<Vec<u8>>, // [y4][x4] — left edge of 4×4 block
    bs_h: Vec<Vec<u8>>, // [y4][x4] — top  edge of 4×4 block
}

impl BsGrid {
    fn new(w4: usize, h4: usize) -> Self {
        Self {
            bs_v: vec![vec![0u8; w4]; h4],
            bs_h: vec![vec![0u8; w4]; h4],
        }
    }
}

/// Boundary strength for a single edge (spec 8.7.2.3).
///   bs=2  either side is intra
///   bs=1  either side has non-zero coefficients
///   bs=0  otherwise (or cross-slice)
fn boundary_strength(p: &BlockState, q: &BlockState) -> u8 {
    if p.slice_id != q.slice_id {
        return 0;
    }
    if p.is_intra || q.is_intra {
        return 2;
    }
    if p.has_nonzero_coeff || q.has_nonzero_coeff {
        return 1;
    }
    // MV-based bs=1 check omitted (no MV data).
    // When MVs are available: return 1 if |mvP−mvQ| ≥ 1 pel or refs differ.
    0
}

fn compute_bs_grid(nt: &NeighborTracker) -> BsGrid {
    let w4 = nt.width_in_units;
    let h4 = nt.height_in_units;
    let mut grid = BsGrid::new(w4, h4);

    for y4 in 0..h4 {
        for x4 in 0..w4 {
            let cur = &nt.blocks[y4 * w4 + x4];
            if x4 > 0 && cur.edge_left {
                let left = &nt.blocks[y4 * w4 + (x4 - 1)];
                grid.bs_v[y4][x4] = boundary_strength(left, cur);
            }
            if y4 > 0 && cur.edge_top {
                let above = &nt.blocks[(y4 - 1) * w4 + x4];
                grid.bs_h[y4][x4] = boundary_strength(above, cur);
            }
        }
    }
    grid
}

// ---------------------------------------------------------------------------
// Sample helpers
// ---------------------------------------------------------------------------

#[inline]
fn clip3(min: i32, max: i32, v: i32) -> i32 {
    v.clamp(min, max)
}

/// Read p-side sample k steps from the edge. base = index of q0.
#[inline]
fn ps(plane: &[u8], base: isize, es: isize, k: isize) -> i32 {
    if let Some(e) = plane.get((base - es * (k + 1)) as usize) {
        i32::from(*e)
    } else {
        0
    }
}

/// Read q-side sample k steps from the edge.
#[inline]
fn qs(plane: &[u8], base: isize, es: isize, k: isize) -> i32 {
    if let Some(e) = plane.get((base + (es * k)) as usize) {
        i32::from(*e)
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Luma filter (spec 8.7.2.4)
// ---------------------------------------------------------------------------

/// Filter a 4-line luma block across one edge.
///
/// `base`        — pixel index of q0 on row 0 of the block
/// `edge_stride` — step along the edge normal (1 for vertical, luma_stride for horizontal)
/// `row_stride`  — step to the next row of the block (luma_stride for vertical, 1 for horizontal)
///
/// The block-level decision (one β check, one strong/normal choice) is made
/// from rows 0 and 3, then applied uniformly to all 4 rows — spec 8.7.2.4.
pub fn filter_luma_block(
    plane: &mut [u8], base: usize, edge_stride: isize, row_stride: isize, bs: u8, beta_val: i32,
    tc_val: i32
) {
    if bs == 0 {
        return;
    }

    let b0 = base as isize;
    let b3 = b0 + 3 * row_stride;
    let es = edge_stride;

    // --- Decision process (spec 8.7.2.5.3) ---
    let row = |plane: &[u8], rb: isize| -> [i32; 8] {
        [
            ps(plane, rb, es, 0),
            ps(plane, rb, es, 1),
            ps(plane, rb, es, 2),
            ps(plane, rb, es, 3),
            qs(plane, rb, es, 0),
            qs(plane, rb, es, 1),
            qs(plane, rb, es, 2),
            qs(plane, rb, es, 3)
        ]
    };
    let r0 = row(plane, b0);
    let r3 = row(plane, b3);

    let dp = |r: &[i32; 8]| (r[2] - 2 * r[1] + r[0]).abs();
    let dq = |r: &[i32; 8]| (r[6] - 2 * r[5] + r[4]).abs();

    let dp0 = dp(&r0);
    let dq0 = dq(&r0);
    let dp3 = dp(&r3);
    let dq3 = dq(&r3);
    let dpq0 = dp0 + dq0;
    let dpq3 = dp3 + dq3;
    let d = dpq0 + dpq3;

    if d >= beta_val {
        return;
    }

    // 8.7.2.5.6: decision for a single line
    let d_sam = |r: &[i32; 8], dpq: i32| -> bool {
        let [p0, _p1, _p2, p3, q0, _q1, _q2, q3] = *r;
        2 * dpq < (beta_val >> 2)
            && (p3 - p0).abs() + (q0 - q3).abs() < (beta_val >> 3)
            && (p0 - q0).abs() < ((5 * tc_val + 1) >> 1)
    };
    let strong = d_sam(&r0, dpq0) && d_sam(&r3, dpq3);

    let side_thr = (beta_val + (beta_val >> 1)) >> 3;
    let do_p1 = (dp0 + dp3) < side_thr;
    let do_q1 = (dq0 + dq3) < side_thr;

    // --- Filtering process (spec 8.7.2.5.7) ---
    for i in 0..4isize {
        let rb = b0 + i * row_stride;
        let [p0, p1, p2, p3, q0, q1, q2, q3] = row(plane, rb);

        if strong {
            let tc2 = 2 * tc_val;
            let p0n = clip3(p0 - tc2, p0 + tc2, (p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3);
            let p1n = clip3(p1 - tc2, p1 + tc2, (p2 + p1 + p0 + q0 + 2) >> 2);
            let p2n = clip3(p2 - tc2, p2 + tc2, (2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3);
            let q0n = clip3(q0 - tc2, q0 + tc2, (p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2 + 4) >> 3);
            let q1n = clip3(q1 - tc2, q1 + tc2, (p0 + q0 + q1 + q2 + 2) >> 2);
            let q2n = clip3(q2 - tc2, q2 + tc2, (p0 + q0 + q1 + 3 * q2 + 2 * q3 + 4) >> 3);

            plane[(rb - es) as usize] = p0n as u8;
            plane[(rb - 2 * es) as usize] = p1n as u8;
            plane[(rb - 3 * es) as usize] = p2n as u8;
            plane[rb as usize] = q0n as u8;
            plane[(rb + es) as usize] = q1n as u8;
            plane[(rb + 2 * es) as usize] = q2n as u8;
        } else {
            let mut delta = (9 * (q0 - p0) - 3 * (q1 - p1) + 8) >> 4;
            if delta.abs() >= tc_val * 10 {
                continue;
            }
            delta = clip3(-tc_val, tc_val, delta);
            plane[(rb - es) as usize] = clip3(0, 255, p0 + delta) as u8;
            plane[rb as usize] = clip3(0, 255, q0 - delta) as u8;

            let tc_half = tc_val >> 1;
            if do_p1 {
                let delta_p = clip3(-tc_half, tc_half, (((p2 + p0 + 1) >> 1) - p1 + delta) >> 1);
                plane[(rb - 2 * es) as usize] = clip3(0, 255, p1 + delta_p) as u8;
            }
            if do_q1 {
                let delta_q = clip3(-tc_half, tc_half, (((q2 + q0 + 1) >> 1) - q1 - delta) >> 1);
                plane[(rb + es) as usize] = clip3(0, 255, q1 + delta_q) as u8;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Chroma filter (spec 8.7.3)
// ---------------------------------------------------------------------------

/// Filter one chroma sample-pair (called once per chroma sample row).
/// Only invoked when bs==2.
fn filter_chroma_samples(plane: &mut [u8], base: usize, stride: isize, tc_val: i32) {
    let rb = base as isize;
    let es = stride;

    let p1 = ps(plane, rb, es, 1);
    let p0 = ps(plane, rb, es, 0);
    let q0 = qs(plane, rb, es, 0);
    let q1 = qs(plane, rb, es, 1);

    // Chroma uses the 4/1 tap formula (spec eq 8-375) — distinct from luma normal.
    let delta = clip3(-tc_val, tc_val, ((q0 - p0) * 4 + p1 - q1 + 4) >> 3);
    plane[(rb - es) as usize] = clip3(0, 255, p0 + delta) as u8;
    plane[rb as usize] = clip3(0, 255, q0 - delta) as u8;
}

// ---------------------------------------------------------------------------
// Main deblock pass
// ---------------------------------------------------------------------------

fn edge_qp(p: &BlockState, q: &BlockState) -> i32 {
    (i32::from(p.qp) + i32::from(q.qp) + 1) >> 1
}

/// Deblock one complete frame.
pub fn deblock_frame(raw: &RawFrame, nt: &NeighborTracker, params: DeblockParams) {
    let grid = compute_bs_grid(nt);
    let w4 = nt.width_in_units;
    let h4 = nt.height_in_units;

    // -----------------------------------------------------------------------
    // Luma — all vertical edges of the picture first, then horizontal edges
    // (spec 8.7.2). Edges lie on the 8x8 luma grid; each edge is decided and
    // filtered in 4-sample segments, so every 4x4 unit along the edge is
    // visited.
    // -----------------------------------------------------------------------
    {
        let mut luma = raw.luma.lock().unwrap();
        let ls = luma.stride as isize;

        for x4 in (2..w4).step_by(2) {
            let x_px = x4 * 4;
            for y4 in 0..h4 {
                let bs = grid.bs_v[y4][x4];
                if bs == 0 {
                    continue;
                }
                let qp = edge_qp(&nt.blocks[y4 * w4 + (x4 - 1)], &nt.blocks[y4 * w4 + x4]);
                let base = offset_plane(&luma, x_px, y4 * 4);
                filter_luma_block(&mut luma.pixels, base, 1, ls, bs, params.beta(qp), params.tc(qp, bs));
            }
        }

        for y4 in (2..h4).step_by(2) {
            let y_px = y4 * 4;
            for x4 in 0..w4 {
                let bs = grid.bs_h[y4][x4];
                if bs == 0 {
                    continue;
                }
                let qp = edge_qp(&nt.blocks[(y4 - 1) * w4 + x4], &nt.blocks[y4 * w4 + x4]);
                let base = offset_plane(&luma, x4 * 4, y_px);
                filter_luma_block(&mut luma.pixels, base, ls, 1, bs, params.beta(qp), params.tc(qp, bs));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Chroma (4:2:0) — edges on the 8x8 *chroma* grid (16 luma samples),
    // only where bs == 2. Each 4-sample chroma segment takes bs/QP from the
    // luma 4x4 unit at the start of its corresponding 8-luma-sample span.
    // -----------------------------------------------------------------------
    let (sub_x, sub_y) = raw.format.get_subsampling();
    if raw.format == crate::hevc_decoder::nal_unit_headers::ChromaFormat::Monochrome
        || sub_x == 0
        || sub_y == 0
    {
        return;
    }
    // chroma grid spacing expressed in 4x4 luma units
    let step_x = (8 * sub_x) / 4;
    let step_y = (8 * sub_y) / 4;
    // number of luma 4x4 units covered by one 4-sample chroma segment
    let seg_x = (4 * sub_x) / 4;
    let seg_y = (4 * sub_y) / 4;

    for plane_idx in 0..2usize {
        let mut cp: MutexGuard<SingleFrame> =
            if plane_idx == 0 { raw.cb.lock().unwrap() } else { raw.cr.lock().unwrap() };
        let qp_off = if plane_idx == 0 { params.cb_qp_offset } else { params.cr_qp_offset };
        let cs = cp.stride as isize;
        let (cw, ch) = (cp.width, cp.height);

        for x4 in (step_x..w4).step_by(step_x) {
            let cx = (x4 * 4) / sub_x;
            for y4 in (0..h4).step_by(seg_y) {
                let bs = grid.bs_v[y4][x4];
                if bs < 2 {
                    continue;
                }
                let cy = (y4 * 4) / sub_y;
                let qp = edge_qp(&nt.blocks[y4 * w4 + (x4 - 1)], &nt.blocks[y4 * w4 + x4]);
                let tc_val = params.tc(chroma_qp(qp, qp_off), 2);
                for dy in 0..4 {
                    if cy + dy >= ch {
                        break;
                    }
                    let base = offset_plane(&cp, cx, cy + dy);
                    filter_chroma_samples(&mut cp.pixels, base, 1, tc_val);
                }
            }
        }

        for y4 in (step_y..h4).step_by(step_y) {
            let cy = (y4 * 4) / sub_y;
            for x4 in (0..w4).step_by(seg_x) {
                let bs = grid.bs_h[y4][x4];
                if bs < 2 {
                    continue;
                }
                let cx = (x4 * 4) / sub_x;
                let qp = edge_qp(&nt.blocks[(y4 - 1) * w4 + x4], &nt.blocks[y4 * w4 + x4]);
                let tc_val = params.tc(chroma_qp(qp, qp_off), 2);
                for dx in 0..4 {
                    if cx + dx >= cw {
                        break;
                    }
                    let base = offset_plane(&cp, cx + dx, cy);
                    filter_chroma_samples(&mut cp.pixels, base, cs, tc_val);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hevc_decoder::constants::PartMode;
    use crate::hevc_decoder::neighbor_tracker::{BlockState, NeighborTracker, PredMode};

    // -----------------------------------------------------------------------
    #[test]
    fn beta_table_monotone() {
        for qp in 1..52i32 {
            assert!(beta(qp) >= beta(qp - 1), "β not monotone at qp={qp}");
        }
    }

    #[test]
    fn tc_table_spot_check() {
        // Verified against ITU-T H.265 Table 8-20
        assert_eq!(tc(20, 1), 1); // idx=20 → 1
        assert_eq!(tc(32, 1), 3); // idx=32 → 3
        assert_eq!(tc(32, 2), 3); // idx=34 → 3
        assert_eq!(tc(35, 1), 4); // idx=35 → 4
        assert_eq!(tc(51, 2), 24); // idx=53 → 24
    }

    #[test]
    fn tc_clamped_below_zero() {
        let _ = tc(-5, 1); // must not panic
    }

    #[test]
    fn chroma_qp_identity_low() {
        for qp in 0..=28i32 {
            assert_eq!(chroma_qp(qp, 0), qp, "chroma_qp identity failed at {qp}");
        }
    }

    #[test]
    fn chroma_qp_top_of_table() {
        // qPi = clamp(51+6, 0, 51) = 51 → QpC = qPi - 6 = 45 (Table 8-10)
        assert_eq!(chroma_qp(51, 6), 45);
        assert_eq!(chroma_qp(51, 0), 45);
    }

    #[test]
    fn chroma_qp_spot_check() {
        // A few known values from spec Table 8-15
        assert_eq!(chroma_qp(30, 0), 29); // qPi=30 → 29
        assert_eq!(chroma_qp(35, 0), 33); // qPi=35 → 33
        assert_eq!(chroma_qp(45, 0), 39); // qPi=45 → 39
    }

    // -----------------------------------------------------------------------
    // Boundary strength
    // -----------------------------------------------------------------------

    fn make_block(is_intra: bool, has_coeff: bool, slice_id: u16) -> BlockState {
        BlockState {
            pred_mode: PredMode::ModeIntra,
            part_mode: PartMode::Part2Nx2N,
            slice_id,
            available: true,
            skip_flag: false,
            cqt_depth: 0,
            is_intra,
            intra_mode_luma: 0,
            intra_mode_chroma: 0,
            is_chroma_dm: false,
            qp: 32,
            has_nonzero_coeff: has_coeff,
            edge_left: true,
            edge_top: true,
        }
    }

    #[test]
    fn bs_both_intra() {
        assert_eq!(
            boundary_strength(&make_block(true, false, 0), &make_block(true, false, 0)),
            2
        );
    }

    #[test]
    fn bs_one_intra() {
        assert_eq!(
            boundary_strength(&make_block(true, false, 0), &make_block(false, false, 0)),
            2
        );
    }

    #[test]
    fn bs_inter_with_coeff() {
        assert_eq!(
            boundary_strength(&make_block(false, true, 0), &make_block(false, false, 0)),
            1
        );
    }

    #[test]
    fn bs_inter_no_coeff() {
        assert_eq!(
            boundary_strength(&make_block(false, false, 0), &make_block(false, false, 0)),
            0
        );
    }

    #[test]
    fn bs_cross_slice_zero() {
        // Different slice_id → no filtering, even if both intra with coeffs
        assert_eq!(
            boundary_strength(&make_block(true, true, 0), &make_block(true, true, 1)),
            0
        );
    }

    // Layout: 4 rows of [p3,p2,p1,p0,q0,q1,q2,q3], each 8 bytes wide.
    // base=4 (q0 of row 0), edge_stride=1, row_stride=8.
    fn make_block_luma(p_val: u8, q_val: u8) -> Vec<u8> {
        let row = [p_val, p_val, p_val, p_val, q_val, q_val, q_val, q_val];
        row.repeat(4)
    }

    #[test]
    fn luma_filter_bs0_no_change() {
        let mut block = make_block_luma(80, 180);
        let original = block.clone();
        filter_luma_block(&mut block, 4, 1, 8, 0, beta(32), tc(32, 0));
        assert_eq!(block, original, "bs=0 must not modify any samples");
    }

    #[test]
    fn luma_filter_no_panic_large_d() {
        let mut block = make_block_luma(100, 150);
        filter_luma_block(&mut block, 4, 1, 8, 2, beta(35), tc(35, 2));
        // p0 is at index 3 on each row; q0 at index 4
        for i in 0..4 {
            assert!(block[i * 8 + 3] >= 100, "p0 row {i} went below original");
            assert!(block[i * 8 + 4] <= 150, "q0 row {i} went above original");
        }
    }

    #[test]
    fn luma_filter_strong_reduces_edge() {
        let mut block = make_block_luma(20, 220);
        let saved = block.clone();
        filter_luma_block(&mut block, 4, 1, 8, 2, beta(51), tc(51, 2));
        for i in 0..4 {
            assert!(
                i32::from(block[i * 8 + 3]) >= i32::from(saved[i * 8 + 3]),
                "p0 row {i} should increase"
            );
            assert!(
                i32::from(block[i * 8 + 4]) <= i32::from(saved[i * 8 + 4]),
                "q0 row {i} should decrease"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Chroma filter
    // -----------------------------------------------------------------------

    #[test]
    fn chroma_filter_moves_toward_average() {
        // Layout: [p1, p0, q0, q1]; base=2 (q0), stride=1
        let mut line = vec![50u8, 50, 200, 200];
        filter_chroma_samples(&mut line, 2, 1, tc(32, 2));
        assert!(line[1] > 50, "p0 should increase");
        assert!(line[2] < 200, "q0 should decrease");
    }

    // -----------------------------------------------------------------------
    // BS grid
    // -----------------------------------------------------------------------

    fn make_nt_uniform(w4: usize, h4: usize, is_intra: bool, slice_id: u16) -> NeighborTracker {
        let blocks = (0..w4 * h4)
            .map(|_| BlockState {
                pred_mode: PredMode::ModeIntra,
                part_mode: PartMode::Part2Nx2N,
                slice_id,
                available: true,
                skip_flag: false,
                cqt_depth: 0,
                is_intra,
                intra_mode_luma: 0,
                intra_mode_chroma: 0,
                is_chroma_dm: false,
                qp: 32,
                has_nonzero_coeff: false,
                edge_left: true,
                edge_top: true,
            })
            .collect();
        NeighborTracker {
            blocks,
            width_in_units: w4,
            height_in_units: h4,
            log2_unit_size: 2,
        }
    }

    #[test]
    fn bs_grid_all_intra_gives_bs2_interior() {
        let nt = make_nt_uniform(4, 4, true, 0);
        let grid = compute_bs_grid(&nt);
        for y4 in 0..4 {
            for x4 in 1..4 {
                assert_eq!(grid.bs_v[y4][x4], 2, "v[{y4}][{x4}]");
            }
        }
        for y4 in 1..4 {
            for x4 in 0..4 {
                assert_eq!(grid.bs_h[y4][x4], 2, "h[{y4}][{x4}]");
            }
        }
    }

    #[test]
    fn bs_grid_left_and_top_boundaries_zero() {
        let nt = make_nt_uniform(4, 4, true, 0);
        let grid = compute_bs_grid(&nt);
        for y4 in 0..4 {
            assert_eq!(grid.bs_v[y4][0], 0, "left column should be 0");
        }
        for x4 in 0..4 {
            assert_eq!(grid.bs_h[0][x4], 0, "top row should be 0");
        }
    }

    #[test]
    fn bs_grid_cross_slice_suppressed() {
        let w4 = 4usize;
        let h4 = 2usize;
        let mut nt = make_nt_uniform(w4, h4, true, 0);
        for y4 in 0..h4 {
            for x4 in (w4 / 2)..w4 {
                nt.blocks[y4 * w4 + x4].slice_id = 1;
            }
        }
        let grid = compute_bs_grid(&nt);
        for y4 in 0..h4 {
            assert_eq!(grid.bs_v[y4][w4 / 2], 0, "cross-slice edge must be bs=0");
        }
    }
}
