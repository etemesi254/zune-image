use std::sync::Arc;

use crate::hevc_decoder::neighbor_tracker::NeighborTracker;
use crate::hevc_decoder::raw_frame::{RawFrame, SingleFrame, offset_plane};
// ==============================================================================
// Spec Tables (ITU-T H.265 Spec 8.7.2.5.3 & Table 8-10)
// ==============================================================================

const BETA_TABLE: [i32; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
    20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52, 54, 56, 58, 60, 62, 64
];

const TC_TABLE: [i32; 54] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 3,
    3, 3, 3, 4, 4, 4, 5, 5, 6, 6, 7, 8, 9, 10, 11, 13, 14, 16, 18, 20, 22, 24
];

// HEVC Chroma QP Mapping Table (Spec Table 8-10)
const CHROMA_QP_MAPPING: [i32; 58] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 29, 30, 31, 32, 33, 33, 34, 34, 35, 35, 36, 36, 37, 37, 38, 39, 40, 41, 42, 43,
    44, 45, 46, 47, 48, 49, 50, 51
];

// ==============================================================================
// 2. The Core Filter Manager
// ==============================================================================

pub fn deblock_frame(
    frame: &Arc<RawFrame>,
    pic_width: usize,
    pic_height: usize,
    tracker: &NeighborTracker,
    pps_cb_qp_offset: i8, // Passed from PPS
    pps_cr_qp_offset: i8  // Passed from PPS
) {
    let mut luma_plane = frame.luma.lock().unwrap();

    // --- Luma Phase 1: Vertical Edges ---
    for x in (8..pic_width).step_by(8) {
        for y in (0..pic_height).step_by(8) {
            let bs = get_boundary_strength(tracker, x, y, x - 1, y);
            if bs == 0 {
                continue;
            }
            let qp = get_edge_qp(tracker, x, y, x - 1, y);
            filter_vertical_luma_edge(&mut luma_plane, x, y, bs, qp);
        }
    }

    // --- Luma Phase 2: Horizontal Edges ---
    for y in (8..pic_height).step_by(8) {
        for x in (0..pic_width).step_by(8) {
            let bs = get_boundary_strength(tracker, x, y, x, y - 1);
            if bs == 0 {
                continue;
            }
            let qp = get_edge_qp(tracker, x, y, x, y - 1);
            filter_horizontal_luma_edge(&mut luma_plane, x, y, bs, qp);
        }
    }

    drop(luma_plane); // Unlock early

    let mut cb_plane = frame.cb.lock().unwrap();
    let mut cr_plane = frame.cr.lock().unwrap();

    // Assuming 4:2:0 subsampling
    let sub_w = pic_width / 2;
    let sub_h = pic_height / 2;

    // --- Chroma Phase 1: Vertical Edges ---
    for x in (8..sub_w).step_by(8) {
        for y in (0..sub_h).step_by(8) {
            let bs = get_boundary_strength(tracker, x * 2, y * 2, (x * 2) - 1, y * 2);
            if bs == 2 {
                let qp_luma = get_edge_qp(tracker, x * 2, y * 2, (x * 2) - 1, y * 2);
                let qp_cb = get_chroma_qp(qp_luma, pps_cb_qp_offset);
                let qp_cr = get_chroma_qp(qp_luma, pps_cr_qp_offset);

                filter_vertical_chroma_edge(&mut cb_plane, x, y, qp_cb);
                filter_vertical_chroma_edge(&mut cr_plane, x, y, qp_cr);
            }
        }
    }

    // --- Chroma Phase 2: Horizontal Edges ---
    for y in (8..sub_h).step_by(8) {
        for x in (0..sub_w).step_by(8) {
            let bs = get_boundary_strength(tracker, x * 2, y * 2, x * 2, (y * 2) - 1);
            if bs == 2 {
                let qp_luma = get_edge_qp(tracker, x * 2, y * 2, x * 2, (y * 2) - 1);
                let qp_cb = get_chroma_qp(qp_luma, pps_cb_qp_offset);
                let qp_cr = get_chroma_qp(qp_luma, pps_cr_qp_offset);

                filter_horizontal_chroma_edge(&mut cb_plane, x, y, qp_cb);
                filter_horizontal_chroma_edge(&mut cr_plane, x, y, qp_cr);
            }
        }
    }
}

// ==============================================================================
// 3. Boundary Strength & QP Derivation
// ==============================================================================

#[inline(always)]
fn get_boundary_strength(
    tracker: &NeighborTracker, q_x: usize, q_y: usize, p_x: usize, p_y: usize
) -> u8 {
    let block_q = tracker.get_state(q_x, q_y);
    let block_p = tracker.get_state(p_x, p_y);

    if !block_p.available || !block_q.available {
        return 0; // Slice boundary or uninitialized
    }

    // Spec 8.7.2.4: If either P or Q is Intra coded, Bs = 2
    if block_p.is_intra || block_q.is_intra {
        return 2;
    }

    // Spec 8.7.2.4: If either has non-zero transform coefficients, Bs = 1
    if block_p.has_nonzero_coeff || block_q.has_nonzero_coeff {
        return 1;
    }

    // NOTE: For full compliance, add Motion Vector & Reference Picture checks here.
    // If MVs differ by >= 1 pixel (4 quarter-pel units), or RefIdx differs -> return 1.

    0
}

#[inline(always)]
fn get_edge_qp(tracker: &NeighborTracker, q_x: usize, q_y: usize, p_x: usize, p_y: usize) -> i32 {
    let block_q = tracker.get_state(q_x, q_y);
    let block_p = tracker.get_state(p_x, p_y);

    // Average QP of the two blocks sharing the edge
    (i32::from(block_p.qp) + i32::from(block_q.qp) + 1) >> 1
}

#[inline(always)]
fn get_chroma_qp(luma_qp: i32, pps_offset: i8) -> i32 {
    let qp_i = (luma_qp + i32::from(pps_offset)).clamp(0, 57);
    CHROMA_QP_MAPPING[qp_i as usize]
}

// ==============================================================================
// 4. Luma Filtering (Vertical & Horizontal)
// ==============================================================================

fn filter_vertical_luma_edge(plane: &mut SingleFrame, x: usize, y: usize, bs: u8, qp: i32) {
    let beta = BETA_TABLE[qp.clamp(0, 51) as usize];
    let mut tc = TC_TABLE[qp.clamp(0, 53) as usize];
    if bs == 2 {
        tc += 1;
    }

    let stride = plane.stride;
    let idx_row0_q0 = offset_plane(plane, x, y);
    let idx_row3_q0 = offset_plane(plane, x, y + 3);

    // Evaluate spatial activity on rows 0 and 3

    let dp0 = (i32::from(plane.pixels[idx_row0_q0 - 3])
        - 2 * i32::from(plane.pixels[idx_row0_q0 - 2])
        + i32::from(plane.pixels[idx_row0_q0 - 1]))
    .abs();
    let dq0 = (i32::from(plane.pixels[idx_row0_q0 + 2])
        - 2 * i32::from(plane.pixels[idx_row0_q0 + 1])
        + i32::from(plane.pixels[idx_row0_q0]))
    .abs();
    let dp3 = (i32::from(plane.pixels[idx_row3_q0 - 3])
        - 2 * i32::from(plane.pixels[idx_row3_q0 - 2])
        + i32::from(plane.pixels[idx_row3_q0 - 1]))
    .abs();
    let dq3 = (i32::from(plane.pixels[idx_row3_q0 + 2])
        - 2 * i32::from(plane.pixels[idx_row3_q0 + 1])
        + i32::from(plane.pixels[idx_row3_q0]))
    .abs();

    let d0 = dp0 + dq0;
    let d3 = dp3 + dq3;

    if (d0 + d3) >= beta {
        return;
    }

    for row in 0..8 {
        let base_q0 = { offset_plane(plane, x, y + row) };

        let p1 = i32::from(plane.pixels[base_q0 - 2]);
        let p0 = i32::from(plane.pixels[base_q0 - 1]);
        let q0 = i32::from(plane.pixels[base_q0]);
        let q1 = i32::from(plane.pixels[base_q0 + 1]);

        let delta = (9 * (q0 - p0) - 3 * (q1 - p1) + 8) >> 4;
        let delta_clipped = delta.clamp(-tc, tc);

        plane.pixels[base_q0 - 1] = (p0 + delta_clipped).clamp(0, 255) as u8;
        plane.pixels[base_q0] = (q0 - delta_clipped).clamp(0, 255) as u8;
    }
}

fn filter_horizontal_luma_edge(plane: &mut SingleFrame, x: usize, y: usize, bs: u8, qp: i32) {
    let beta = BETA_TABLE[qp.clamp(0, 51) as usize];
    let mut tc = TC_TABLE[qp.clamp(0, 53) as usize];
    if bs == 2 {
        tc += 1;
    }

    let stride = plane.stride;

    let idx_col0_q0 = offset_plane(plane, x, y);
    let idx_col3_q0 = offset_plane(plane, x + 3, y);

    let dp0 = (i32::from(plane.pixels[idx_col0_q0 - 3 * stride])
        - 2 * i32::from(plane.pixels[idx_col0_q0 - 2 * stride])
        + i32::from(plane.pixels[idx_col0_q0 - stride]))
    .abs();
    let dq0 = (i32::from(plane.pixels[idx_col0_q0 + 2 * stride])
        - 2 * i32::from(plane.pixels[idx_col0_q0 + stride])
        + i32::from(plane.pixels[idx_col0_q0]))
    .abs();
    let dp3 = (i32::from(plane.pixels[idx_col3_q0 - 3 * stride])
        - 2 * i32::from(plane.pixels[idx_col3_q0 - 2 * stride])
        + i32::from(plane.pixels[idx_col3_q0 - stride]))
    .abs();
    let dq3 = (i32::from(plane.pixels[idx_col3_q0 + 2 * stride])
        - 2 * i32::from(plane.pixels[idx_col3_q0 + stride])
        + i32::from(plane.pixels[idx_col3_q0]))
    .abs();

    let d0 = dp0 + dq0;
    let d3 = dp3 + dq3;

    if (d0 + d3) >= beta {
        return;
    }

    for col in 0..8 {
        let base_q0 = offset_plane(plane, x + col, y);

        let p2 = i32::from(plane.pixels[base_q0 - 3 * stride]);
        let p1 = i32::from(plane.pixels[base_q0 - 2 * stride]);
        let p0 = i32::from(plane.pixels[base_q0 - stride]);
        let q0 = i32::from(plane.pixels[base_q0]);
        let q1 = i32::from(plane.pixels[base_q0 + stride]);
        let q2 = i32::from(plane.pixels[base_q0 + 2 * stride]);

        let delta = (9 * (q0 - p0) - 3 * (q1 - p1) + 8) >> 4;
        let delta_clipped = delta.clamp(-tc, tc);

        plane.pixels[base_q0 - stride] = (p0 + delta_clipped).clamp(0, 255) as u8;
        plane.pixels[base_q0] = (q0 - delta_clipped).clamp(0, 255) as u8;
    }
}

// ==============================================================================
// 5. Chroma Filtering (Vertical & Horizontal)
// ==============================================================================

fn filter_vertical_chroma_edge(plane: &mut SingleFrame, x_c: usize, y_c: usize, qp_c: i32) {
    let tc = TC_TABLE[(qp_c + 2).clamp(0, 53) as usize];
    if tc <= 0 {
        return;
    }

    for row in 0..8 {
        let base_q0 = offset_plane(plane, x_c, y_c + row);

        let p1 = i32::from(plane.pixels[base_q0 - 2]);
        let p0 = i32::from(plane.pixels[base_q0 - 1]);
        let q0 = i32::from(plane.pixels[base_q0]);
        let q1 = i32::from(plane.pixels[base_q0 + 1]);

        let delta = ((((q0 - p0) << 2) + p1 - q1 + 4) >> 3).clamp(-tc, tc);

        plane.pixels[base_q0 - 1] = (p0 + delta).clamp(0, 255) as u8;
        plane.pixels[base_q0] = (q0 - delta).clamp(0, 255) as u8;
    }
}

fn filter_horizontal_chroma_edge(plane: &mut SingleFrame, x_c: usize, y_c: usize, qp_c: i32) {
    let tc = TC_TABLE[(qp_c + 2).clamp(0, 53) as usize];
    if tc <= 0 {
        return;
    }

    let stride = plane.stride;

    for col in 0..8 {
        let base_q0 = offset_plane(plane, x_c + col, y_c);

        let p1 = i32::from(plane.pixels[base_q0 - 2 * stride]);
        let p0 = i32::from(plane.pixels[base_q0 - stride]);
        let q0 = i32::from(plane.pixels[base_q0]);
        let q1 = i32::from(plane.pixels[base_q0 + stride]);

        let delta = ((((q0 - p0) << 2) + p1 - q1 + 4) >> 3).clamp(-tc, tc);

        plane.pixels[base_q0 - stride] = (p0 + delta).clamp(0, 255) as u8;
        plane.pixels[base_q0] = (q0 - delta).clamp(0, 255) as u8;
    }
}
