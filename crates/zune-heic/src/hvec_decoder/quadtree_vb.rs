use zune_core::log::trace;

use crate::hvec_decoder::DEBUG_MORE;
use crate::hvec_decoder::binarizer::Binarizer;
use crate::hvec_decoder::context_model::{BlockState, NeighborTracker};
use crate::hvec_decoder::nal_unit_headers::{Pps, SliceType, Sps};

const BASE_CTX_SPLIT_FLAG: usize = 0;
const BASE_CTX_TRANS_SUBDIV_FLAG: usize = 36;
const BASE_CTX_QT_CBF_LUMA: usize = 39;
const BASE_CTX_QT_CBF_CHROMA: usize = 44;
// Contexts from HEVC Spec Table 9-37
const BASE_CTX_IPRED_LUMA: usize = 1;
const BASE_CTX_IPRED_CHROMA: usize = 64;

pub fn decode_pu_intra(binarizer: &mut Binarizer) -> u8 {
    // 1. Luma Mode
    let prev_intra_luma_pred_flag = binarizer.engine.decode_decision(BASE_CTX_IPRED_LUMA);
    if prev_intra_luma_pred_flag == 1 {
        // MPM Index: 1 bit (bypass) if only 3 MPMs, or more if expanded
        let _mpm_idx = binarizer.engine.decode_bypass();
    } else {
        // Remainder: 5 bits fixed-length bypass
        let _rem_mode = binarizer.engine.decode_bypass_n(5);
    }

    // 2. Chroma Mode
    if binarizer.engine.decode_decision(BASE_CTX_IPRED_CHROMA) == 1 {
        let _chroma_mode_idx = binarizer.engine.decode_bypass_n(2);
    }

    0 // Return a dummy mode for now
}

pub fn decode_coding_quadtree(
    binarizer: &mut Binarizer, tracker: &mut NeighborTracker, sps: &Sps, pps: &Pps,
    slice_type: SliceType, x0: usize, y0: usize, size: usize, depth: u8
) {
    let mut split_cu_flag = false;

    // Check if we must split or can split
    if x0 + size > sps.pic_width_in_luma_samples as usize
        || y0 + size > sps.pic_height_in_luma_samples as usize
    {
        split_cu_flag = true;
    } else if size > sps.min_cb_size_y as usize {
        let ctx = tracker.get_split_ctx(x0, y0, depth);
        split_cu_flag = binarizer.engine.decode_decision(ctx) == 1;
        if DEBUG_MORE {
            println!("SplitFlag val={}", split_cu_flag as u8);
        }
    }

    if split_cu_flag {
        let half = size / 2;
        decode_coding_quadtree(
            binarizer,
            tracker,
            sps,
            pps,
            slice_type,
            x0,
            y0,
            half,
            depth + 1
        );
        decode_coding_quadtree(
            binarizer,
            tracker,
            sps,
            pps,
            slice_type,
            x0 + half,
            y0,
            half,
            depth + 1
        );
        decode_coding_quadtree(
            binarizer,
            tracker,
            sps,
            pps,
            slice_type,
            x0,
            y0 + half,
            half,
            depth + 1
        );
        decode_coding_quadtree(
            binarizer,
            tracker,
            sps,
            pps,
            slice_type,
            x0 + half,
            y0 + half,
            half,
            depth + 1
        );
    } else {
        // --- THIS IS A LEAF CU ---
        // 1. Record the depth in the tracker for neighbors to use
        tracker.set_split(x0, y0, size, depth);

        // 2. Decode the Prediction Unit (PU) data
        // This clears the bits that were causing your "fishy" transform flags
        if slice_type == SliceType::I {
            decode_pu_intra(binarizer);
        }

        // 3. Decode the Transform Tree
        decode_transform_tree(
            binarizer,
            x0,
            y0,
            size.trailing_zeros() as u8, // log2_size
            0,                           // trafo_depth
            true,                        // parent_cbf_cb
            true,                        // parent_cbf_cr
            0                            // blk_idx
        );
    }
}

// Mock constants for context base indices. In a full decoder, these are
// usually mapped via an enum or an offset table based on the HEVC spec.
const BASE_CTX_SKIP_FLAG: usize = 27;
const BASE_CTX_PRED_MODE: usize = 25; // Intra vs Inter
const BASE_CTX_PART_MODE: usize = 26; // Partitioning mode

pub fn decode_coding_unit(
    binarizer: &mut Binarizer,
    tracker: &mut NeighborTracker,
    sps: &Sps,
    pps: &Pps,
    slice_type: SliceType, // Pass this down from the SliceHeader!
    x0: usize,
    y0: usize,
    size: usize,
    depth: u8
) {
    let mut is_skipped = false;
    let mut is_intra = true; // I-slices are always Intra

    // ========================================================================
    // 1. Skip Flag (Only present in P and B slices)
    // ========================================================================
    if slice_type != SliceType::I {
        let skip_ctx_inc = tracker.derive_skip_context(x0, y0);
        let skip_ctx_idx = BASE_CTX_SKIP_FLAG + skip_ctx_inc;

        is_skipped = binarizer.engine.decode_decision(skip_ctx_idx) == 1;
    }

    if is_skipped {
        // A skipped CU means:
        // - It is an Inter block (is_intra = false).
        // - Its partition mode is automatically 2Nx2N (the whole block).
        // - It uses the Merge mode for motion vectors.
        // - It has NO residual pixel data (cbf = 0).

        // TODO: Read the `merge_idx` to figure out which neighbor's motion
        // vector to copy. For now, we just record its state and bail out!

        tracker.update_block(
            x0,
            y0,
            size,
            BlockState {
                available:       true,
                skip_flag:       true,
                cqt_depth:       depth,
                is_intra:        false,
                intra_mode_luma: 0
            }
        );

        return; // We are done with this CU!
    }

    // ========================================================================
    // 2. Prediction Mode (Intra vs Inter)
    // ========================================================================
    if slice_type != SliceType::I {
        // We only read pred_mode_flag if we are in a P/B slice.
        // 1 = Intra, 0 = Inter. Context derivation for this is usually flat (ctxInc = 0).
        let pred_mode_bin = binarizer.engine.decode_decision(BASE_CTX_PRED_MODE);
        is_intra = pred_mode_bin == 1;
    }

    // ========================================================================
    // 3. Partition Mode (part_mode)
    // ========================================================================
    // This tells us if the CU is further divided into smaller Prediction Units (PUs).
    // For example, a 16x16 CU might have a part_mode of 2NxN, meaning it is split
    // horizontally into two 16x8 PUs, each with their own motion vectors.

    // Binarizing part_mode is complex and depends heavily on `size` and `is_intra`.
    // I-slices are usually 2Nx2N (or NxN if at minimum CU size).
    // Inter-slices can be 2Nx2N, 2NxN, Nx2N, NxN, or even asymmetric (AMP).
    let part_mode = decode_part_mode(binarizer, sps, size, is_intra);

    // ========================================================================
    // 4. Prediction Unit (PU) Parsing
    // ========================================================================
    let mut luma_mode = 0;

    if is_intra {
       // let (l, c) = decode_pu_intra(binarizer, tracker, x0, y0);
        //luma_mode = l;
        // In a real decoder, you would store `c` (chroma mode) in your TU tree
        // for when you render the YUV color planes.
    } else {
        // decode_pu_inter(...)
    }
    // ========================================================================
    // 5. Transform Unit (TU) Parsing (The Residuals)
    // ========================================================================
    // Unless we are skipped, we have to check if there are residual pixels to add.
    // Read the `rqm_root_cbf` (Root Coded Block Flag).
    // If 1, dive into the Transform Tree to read the Truncated Rice coefficients!

    let log2_trafo_size = (size as f64).log2() as u8;

    // Only parse the transform tree if it's not a skipped block!
    if !is_skipped {
        decode_transform_tree(
            binarizer,
            x0,
            y0,
            log2_trafo_size,
            0,    // starting trafo_depth
            true, // Root parent_cbf_cb is implicitly true
            true, // Root parent_cbf_cr is implicitly true,
            0
        );
    }

    // ========================================================================
    // 6. Finalize and Update Tracker
    // ========================================================================
    tracker.update_block(
        x0,
        y0,
        size,
        BlockState {
            available: true,
            skip_flag: false,
            cqt_depth: depth,
            is_intra,
            intra_mode_luma: luma_mode
        }
    );
}

// A placeholder for the partition mode binarization
fn decode_part_mode(binarizer: &mut Binarizer, sps: &Sps, size: usize, is_intra: bool) -> u8 {
    // Implementing the exact binarization tree for part_mode requires a few checks
    // against the SPS (like amp_enabled_flag).
    // For now, we assume 2Nx2N (which is represented as mode 0 in HEVC).
    0
}

// Add this to your syntax/quadtree module

// Base context for prev_intra_luma_pred_flag (usually offset in the real spec)
const BASE_CTX_PREV_INTRA_LUMA: usize = 30;
const BASE_CTX_INTRA_CHROMA: usize = 31;

pub fn decode_transform_tree(
    binarizer: &mut Binarizer,
    x0: usize,
    y0: usize,
    log2_trafo_size: u8,
    trafo_depth: u8,
    parent_cbf_cb: bool, // Passed down from parent! (Starts as true at root)
    parent_cbf_cr: bool, // Passed down from parent! (Starts as true at root)
    blk_idx: u8          // ADDED: Which of the 4 children are we? (0 for root)
) {
    let mut split_transform_flag = false;

    // In a full implementation, you'd check max_transform_hierarchy_depth
    // and log2_min_transform_block_size from the SPS here.
    let can_split = log2_trafo_size > 2; // Assume min TU size is 4x4 (log2 of 4 is 2)

    if can_split {
        // Context for splitting a TU is exactly its current transform depth (capped at 2)
        let subdiv_ctx_inc = trafo_depth.min(2) as usize;
        let subdiv_ctx = BASE_CTX_TRANS_SUBDIV_FLAG + subdiv_ctx_inc;

        split_transform_flag = binarizer.engine.decode_decision(subdiv_ctx) == 1;

        if DEBUG_MORE {
            println!(
                "parseTransformSubdivFlag() symbol={}   ctx={}",
                split_transform_flag as u8, subdiv_ctx_inc
            );
        }
    }

    // ========================================================================
    // CHROMA RULES
    // Chroma can NEVER go below 4x4 (log2 = 2).
    // If Luma is 4x4, we only process Chroma on the 0th block!
    // ========================================================================
    let log2_trafo_size_c = std::cmp::max(2, log2_trafo_size.saturating_sub(1));
    let process_chroma = (log2_trafo_size > 2) || (log2_trafo_size == 2 && blk_idx == 0);

    let mut cbf_cb = false;
    let mut cbf_cr = false;

    if process_chroma && (parent_cbf_cb || parent_cbf_cr) {
        // Chroma CBF context is simply the transform depth!
        let chroma_ctx_inc = trafo_depth as usize;
        let chroma_ctx = BASE_CTX_QT_CBF_CHROMA + chroma_ctx_inc;

        if parent_cbf_cb {
            cbf_cb = binarizer.engine.decode_decision(chroma_ctx) == 1;
            if DEBUG_MORE {
                println!(
                    "parseQtCbf()   symbol={}   ctx={}  etype=1",
                    cbf_cb as u8, chroma_ctx_inc
                );
            }
        }

        if parent_cbf_cr {
            cbf_cr = binarizer.engine.decode_decision(chroma_ctx) == 1;
            if DEBUG_MORE {
                println!(
                    "parseQtCbf()   symbol={}   ctx={}  etype=2",
                    cbf_cr as u8, chroma_ctx_inc
                );
            }
        }
    } else if !process_chroma {
        // If we are blocks 1, 2, or 3 of a 4x4 Luma split, we simply inherit
        // the Chroma CBFs from block 0!
        cbf_cb = parent_cbf_cb;
        cbf_cr = parent_cbf_cr;
    }

    // ========================================================================
    // SPLIT RECURSION
    // ========================================================================
    if split_transform_flag {
        let half_size = 1 << (log2_trafo_size - 1);

        // Notice how we pass our cbf_cb and cbf_cr down to the children!
        // We also pass 0, 1, 2, 3 so the children know their quadrant index.
        decode_transform_tree(
            binarizer,
            x0,
            y0,
            log2_trafo_size - 1,
            trafo_depth + 1,
            cbf_cb,
            cbf_cr,
            0
        );
        decode_transform_tree(
            binarizer,
            x0 + half_size,
            y0,
            log2_trafo_size - 1,
            trafo_depth + 1,
            cbf_cb,
            cbf_cr,
            1
        );
        decode_transform_tree(
            binarizer,
            x0,
            y0 + half_size,
            log2_trafo_size - 1,
            trafo_depth + 1,
            cbf_cb,
            cbf_cr,
            2
        );
        decode_transform_tree(
            binarizer,
            x0 + half_size,
            y0 + half_size,
            log2_trafo_size - 1,
            trafo_depth + 1,
            cbf_cb,
            cbf_cr,
            3
        );

        return; // Important! We don't read Luma CBF on split nodes, only leaf nodes.
    }

    // ========================================================================
    // LUMA CBF (Y) - Only decoded at LEAF nodes!
    // ========================================================================

    // Context derivation for Luma CBF:
    // If trafo_depth is 0, ctx = 1. If trafo_depth > 0, ctx = 0.
    let luma_ctx_inc = if trafo_depth == 0 { 1 } else { 0 };
    let luma_ctx = BASE_CTX_QT_CBF_LUMA + luma_ctx_inc;

    let cbf_luma = binarizer.engine.decode_decision(luma_ctx) == 1;

    if DEBUG_MORE {
        println!(
            "parseQtCbf()   symbol={}   ctx={}  etype=0",
            cbf_luma as u8, luma_ctx_inc
        );
    }

    // ========================================================================
    // THE RESIDUALS (The Math Math)
    // ========================================================================
    if cbf_luma {
        decode_residual(binarizer, x0, y0, log2_trafo_size, 0 /* Luma */);
    }

    // Only parse Chroma residuals if we are allowed to process Chroma here!
    if process_chroma {
        if cbf_cb {
            decode_residual(binarizer, x0, y0, log2_trafo_size_c, 1 /* Cb */);
        }
        if cbf_cr {
            decode_residual(binarizer, x0, y0, log2_trafo_size_c, 2 /* Cr */);
        }
    }
}

/// HEVC Spec: Derivation process for ctxInc for sig_coeff_flag
fn derive_sig_coeff_ctx_inc(
    cg_x: usize, cg_y: usize, pos_x: usize, pos_y: usize, tu_size: usize, color_idx: u8,
    cg_tracker: &CgTracker
) -> usize {
    // 1. If we are a 4x4 TU, context derivation is incredibly simple.
    if tu_size == 4 {
        // Look up the context based on the pixel's coordinate inside the 4x4 block
        let ctx_idx_map: [[usize; 4]; 4] = [[0, 1, 4, 5], [2, 3, 4, 5], [6, 6, 8, 8], [7, 7, 8, 8]];
        return ctx_idx_map[pos_y][pos_x] + (if color_idx > 0 { 15 } else { 0 });
    }

    // 2. If we are larger than 4x4, we have to look at neighboring Coefficient Groups
    let right_cg_coded = if cg_x < cg_tracker.num_cg_x - 1 {
        cg_tracker.flags[cg_y][cg_x + 1]
    } else {
        false
    };
    let bottom_cg_coded = if cg_y < cg_tracker.num_cg_y - 1 {
        cg_tracker.flags[cg_y + 1][cg_x]
    } else {
        false
    };

    // Pattern ID depends on neighbors
    let sig_pattern = (right_cg_coded as usize) + ((bottom_cg_coded as usize) << 1);

    // Context offset based on where we are inside the 4x4 block
    let pos_in_cg_x = pos_x % 4;
    let pos_in_cg_y = pos_y % 4;
    let is_bottom_right = pos_in_cg_x + pos_in_cg_y >= 3;

    // A simplified version of the massive HM lookup tables for Luma (color_idx == 0)
    let ctx_offset = if color_idx == 0 {
        if tu_size == 8 {
            if sig_pattern == 0 {
                if is_bottom_right { 2 } else { 1 }
            } else if sig_pattern == 1 {
                if pos_in_cg_y == 0 { 4 } else { 5 }
            } else if sig_pattern == 2 {
                if pos_in_cg_x == 0 { 7 } else { 8 }
            } else {
                10
            }
        } else {
            // 16x16 or 32x32
            if sig_pattern == 0 {
                if is_bottom_right { 13 } else { 12 }
            } else if sig_pattern == 1 {
                if pos_in_cg_y == 0 { 15 } else { 16 }
            } else if sig_pattern == 2 {
                if pos_in_cg_x == 0 { 18 } else { 19 }
            } else {
                21
            }
        }
    } else {
        // Chroma is much simpler
        if sig_pattern == 0 {
            15
        } else if sig_pattern == 1 {
            16
        } else if sig_pattern == 2 {
            16
        } else {
            17
        }
    };

    // If we are at DC (0,0), it has a dedicated context
    if pos_x == 0 && pos_y == 0 {
        if color_idx == 0 { 0 } else { 15 } // DC context
    } else {
        ctx_offset
    }
}
pub fn decode_residual(
    binarizer: &mut Binarizer,
    x0: usize,
    y0: usize,
    log2_trafo_size: u8,
    color_idx: u8 // 0 = Luma, 1 = Cb, 2 = Cr
) {
    let size = 1 << log2_trafo_size;

    // --- Trace exactly like HM line 265 ---
    if DEBUG_MORE {
        // HM prints: parseCoeffNxN() eType=0 width=32 height=32 ...
        println!(
            "parseCoeffNxN()    eType={}    width={}   height={}  depth=... (truncated for clarity)",
            color_idx, size, size
        );
    }

    // ========================================================================
    // Step 1: Last Significant Coefficient
    // ========================================================================
    let base_ctx_x = if color_idx == 0 { 60 } else { 90 };
    let base_ctx_y = if color_idx == 0 { 75 } else { 105 };
    let ctx_offset = match log2_trafo_size {
        2 => 0,
        3 => 3,
        4 => 6,
        5 => 10,
        _ => 0
    };

    let mut last_sig_x = 0;
    let mut last_sig_y = 0;

    while binarizer
        .engine
        .decode_decision(base_ctx_x + ctx_offset + last_sig_x)
        == 1
    {
        last_sig_x += 1;
    }
    while binarizer
        .engine
        .decode_decision(base_ctx_y + ctx_offset + last_sig_y)
        == 1
    {
        last_sig_y += 1;
    }

    // Suffixes (if prefix > 3)
    if last_sig_x > 3 {
        let suffix_len = (last_sig_x >> 1) - 1;
        let suffix = binarizer.decode_fl(suffix_len as u8) as usize;
        last_sig_x = (1 << suffix_len) + 2 + (last_sig_x & 1) * (1 << suffix_len) + suffix;
    }
    if last_sig_y > 3 {
        let suffix_len = (last_sig_y >> 1) - 1;
        let suffix = binarizer.decode_fl(suffix_len as u8) as usize;
        last_sig_y = (1 << suffix_len) + 2 + (last_sig_y & 1) * (1 << suffix_len) + suffix;
    }

    // --- Trace LastSigCoeff ---
    if DEBUG_MORE {
        println!("LastSigCoeff: X={}, Y={}", last_sig_x, last_sig_y);
    }
    // ========================================================================
    // Step 2: The Reverse Scan Loop
    // ========================================================================
    let last_cg_x = last_sig_x / 4;
    let last_cg_y = last_sig_y / 4;
    let last_subblock_idx = get_subblock_index(last_cg_x, last_cg_y, size);

    let mut cg_tracker = CgTracker::new(size, color_idx);
    let mut c_rice_param = 0;
    let mut final_coefficients = vec![0i32; size * size]; // Store them flat to print later

    for subblock_idx in (0..=last_subblock_idx).rev() {
        let (cg_x, cg_y) = get_cg_coordinates(subblock_idx, size);
        let mut coded_sub_block_flag = false;

        if subblock_idx == last_subblock_idx || subblock_idx == 0 {
            coded_sub_block_flag = true;
        } else {
            let csbf_ctx = cg_tracker.derive_csbf_context(cg_x, cg_y);
            coded_sub_block_flag = binarizer.engine.decode_decision(csbf_ctx) == 1;
        }

        cg_tracker.set_coded(cg_x, cg_y, coded_sub_block_flag);

        if coded_sub_block_flag {
            // ========================================================================
            // Step 3: Passes 1-5
            // ========================================================================
            let mut sig_coeff_flags = [false; 16];
            let mut greater1_flags = [false; 16];
            let mut greater2_flags = [false; 16];
            let mut absolute_values = [0u32; 16];
            let mut signs = [0u8; 16];

            let mut num_greater1_flags_decoded = 0;
            let mut first_greater1_index = -1;

            // --- PASS 1: sig_coeff_flag ---
            // Base context offset for the actual flags
            let base_ctx_sig = if color_idx == 0 { 120 } else { 150 };

            for n in (0..16).rev() {
                let (px, py) = get_cg_coordinates(n, 4); // 4x4 internal scan
                let global_x = (cg_x * 4) + px;
                let global_y = (cg_y * 4) + py;

                if global_x == last_sig_x && global_y == last_sig_y {
                    sig_coeff_flags[n] = true;
                } else if global_x <= last_sig_x && global_y <= last_sig_y {
                    // ACTUALLY DERIVE THE CONTEXT NOW!
                    let sig_ctx_inc = derive_sig_coeff_ctx_inc(
                        cg_x,
                        cg_y,
                        global_x,
                        global_y,
                        size,
                        color_idx,
                        &cg_tracker
                    );
                    sig_coeff_flags[n] =
                        binarizer.engine.decode_decision(base_ctx_sig + sig_ctx_inc) == 1;
                }
            }

            // --- PASS 2: greater1_flag ---
            let base_ctx_gr1 = if color_idx == 0 { 165 } else { 185 };
            // (In a real decoder, the context index here depends on cRiceParam and subSet idx)
            let gr1_ctx = base_ctx_gr1;

            for n in (0..16).rev() {
                if sig_coeff_flags[n] && num_greater1_flags_decoded < 8 {
                    greater1_flags[n] = binarizer.engine.decode_decision(gr1_ctx) == 1;
                    num_greater1_flags_decoded += 1;
                    if greater1_flags[n] && first_greater1_index == -1 {
                        first_greater1_index = n as i32;
                    }
                }
            }

            // --- PASS 3: greater2_flag ---
            let base_ctx_gr2 = if color_idx == 0 { 190 } else { 205 };
            if first_greater1_index != -1 {
                greater2_flags[first_greater1_index as usize] =
                    binarizer.engine.decode_decision(base_ctx_gr2) == 1;
            }

            // --- PASS 4: sign_flag (BYPASS) ---
            for n in (0..16).rev() {
                if sig_coeff_flags[n] {
                    signs[n] = binarizer.engine.decode_bypass();
                }
            }

            // --- PASS 5: coeff_abs_level_remaining (BYPASS TR) ---
            for n in (0..16).rev() {
                if sig_coeff_flags[n] {
                    let base_level = 1 + (greater1_flags[n] as u32) + (greater2_flags[n] as u32);
                    let needs_remaining = greater2_flags[n]
                        || (greater1_flags[n] && n as i32 != first_greater1_index)
                        || (num_greater1_flags_decoded == 8);

                    if needs_remaining {
                        let remaining = binarizer.decode_tr(c_rice_param, 4);
                        absolute_values[n] = base_level + remaining;
                        if absolute_values[n] > (3 * (1 << c_rice_param)) {
                            c_rice_param = (c_rice_param + 1).min(4);
                        }
                    } else {
                        absolute_values[n] = base_level;
                    }

                    // Map it to our flat array for printing!
                    let (px, py) = get_cg_coordinates(n, 4);
                    let flat_idx = ((cg_y * 4) + py) * size + ((cg_x * 4) + px);

                    let final_val = if signs[n] == 1 {
                        -(absolute_values[n] as i32)
                    } else {
                        absolute_values[n] as i32
                    };
                    final_coefficients[flat_idx] = final_val;
                }
            }
        }
    }

    // --- Trace Coefficients exactly like HM printSBACCoeffData ---
    if DEBUG_MORE {
        for (i, &coeff) in final_coefficients.iter().enumerate() {
            if coeff != 0 {
                // HM prints the 1D scan index, we'll print the Z-scan equivalent or just the flat index
                println!(
                    "Coeff[{}] = {}{}",
                    i,
                    if coeff < 0 { "" } else { "+" },
                    coeff
                );
            }
        }
    }
}

const BASE_CTX_SAO_MERGE: usize = 200; // Put these at the top of your contexts
const BASE_CTX_SAO_TYPE_IDX: usize = 201;

pub fn decode_sao(
    binarizer: &mut Binarizer,
    rx: usize, // CTU X coordinate (0, 1, 2...)
    ry: usize, // CTU Y coordinate (0, 1, 2...)
    slice_sao_luma: bool,
    slice_sao_chroma: bool
) {
    let mut is_merged = false;

    // 1. Merge Left (Only if not on the left edge)
    if rx > 0 {
        is_merged = binarizer.engine.decode_decision(BASE_CTX_SAO_MERGE) == 1;
    }
    // 2. Merge Up (Only if not on the top edge, and didn't merge left)
    if ry > 0 && !is_merged {
        is_merged = binarizer.engine.decode_decision(BASE_CTX_SAO_MERGE) == 1;
    }

    // 3. If no merge, read the SAO types for this CTU
    if !is_merged {
        if slice_sao_luma {
            let mut type_idx = 0;
            if binarizer.engine.decode_decision(BASE_CTX_SAO_TYPE_IDX) == 1 {
                type_idx = 1 + binarizer.engine.decode_bypass();
            }
            if type_idx > 0 {
                // If this panics, it means SAO is actually active and we need to
                // write the rest of the offset bypass bins!
                panic!(
                    "SAO Luma Type {} encountered! Need full SAO parser.",
                    type_idx
                );
            }
        }
        if slice_sao_chroma {
            let mut type_idx = 0;
            if binarizer.engine.decode_decision(BASE_CTX_SAO_TYPE_IDX) == 1 {
                type_idx = 1 + binarizer.engine.decode_bypass();
            }
            if type_idx > 0 {
                panic!(
                    "SAO Chroma Type {} encountered! Need full SAO parser.",
                    type_idx
                );
            }
        }
    }
}
const BASE_CTX_CSBF_LUMA: usize = 42;
const BASE_CTX_CSBF_CHROMA: usize = 44;

/// Tracks the "Coded Sub-Block Flags" (CSBF) inside a single Transform Unit.
/// A TU can be up to 32x32 pixels, meaning it contains up to an 8x8 grid of 4x4 Coefficient Groups.
pub struct CgTracker {
    // True if the 4x4 group at [y][x] has at least one non-zero coefficient.
    flags:     [[bool; 8]; 8],
    num_cg_x:  usize,
    num_cg_y:  usize,
    color_idx: u8
}

impl CgTracker {
    pub fn new(tu_size: usize, color_idx: u8) -> Self {
        Self {
            flags: [[false; 8]; 8],
            num_cg_x: tu_size / 4,
            num_cg_y: tu_size / 4,
            color_idx
        }
    }

    /// Records whether a specific 4x4 group had any active coefficients.
    #[inline]
    pub fn set_coded(&mut self, cg_x: usize, cg_y: usize, is_coded: bool) {
        self.flags[cg_y][cg_x] = is_coded;
    }

    /// Derives the exact CABAC context index for the `coded_sub_block_flag`
    /// based on the Right and Bottom neighbors.
    pub fn derive_csbf_context(&self, cg_x: usize, cg_y: usize) -> usize {
        let mut ctx_inc = 0;

        // Did the block to our right have paint?
        if cg_x < self.num_cg_x - 1 && self.flags[cg_y][cg_x + 1] {
            ctx_inc += 1;
        }

        // Did the block below us have paint?
        if cg_y < self.num_cg_y - 1 && self.flags[cg_y + 1][cg_x] {
            ctx_inc += 1;
        }

        let base_ctx = if self.color_idx == 0 { BASE_CTX_CSBF_LUMA } else { BASE_CTX_CSBF_CHROMA };

        base_ctx + ctx_inc
    }
}
// ============================================================================
// HEVC Up-Right Diagonal Scan Tables
// Maps a 1D index to a 2D (x, y) coordinate.
// ============================================================================

/// 2x2 Grid (Used for 8x8 Transform Units)
#[rustfmt::skip]
const DIAG_SCAN_2X2: [(usize, usize); 4] = [
    (0, 0), (0, 1), (1, 0), (1, 1)
];

/// 4x4 Grid (Used for 16x16 Transform Units)
#[rustfmt::skip]
const DIAG_SCAN_4X4: [(usize, usize); 16] = [
    (0, 0), (0, 1), (1, 0), (0, 2), (1, 1), (2, 0), (0, 3), (1, 2),
    (2, 1), (3, 0), (1, 3), (2, 2), (3, 1), (2, 3), (3, 2), (3, 3)
];

/// 8x8 Grid (Used for 32x32 Transform Units)
#[rustfmt::skip]
const DIAG_SCAN_8X8: [(usize, usize); 64] = [
    (0,0), (0,1), (1,0), (0,2), (1,1), (2,0), (0,3), (1,2),
    (2,1), (3,0), (0,4), (1,3), (2,2), (3,1), (4,0), (0,5),
    (1,4), (2,3), (3,2), (4,1), (5,0), (0,6), (1,5), (2,4),
    (3,3), (4,2), (5,1), (6,0), (0,7), (1,6), (2,5), (3,4),
    (4,3), (5,2), (6,1), (7,0), (1,7), (2,6), (3,5), (4,4),
    (5,3), (6,2), (7,1), (2,7), (3,6), (4,5), (5,4), (6,3),
    (7,2), (3,7), (4,6), (5,5), (6,4), (7,3), (4,7), (5,6),
    (6,5), (7,4), (5,7), (6,6), (7,5), (6,7), (7,6), (7,7)
];

// ============================================================================
// Scan Mapping Functions
// ============================================================================

/// Translates a 1D reverse-scan index into a 2D (X, Y) coordinate
/// for the 4x4 Coefficient Group (CG).
pub fn get_cg_coordinates(index: usize, tu_size: usize) -> (usize, usize) {
    match tu_size {
        4 => (0, 0),                // A 4x4 TU is exactly one 4x4 CG. Index is always 0.
        8 => DIAG_SCAN_2X2[index],  // 8x8 TU is a 2x2 grid of CGs
        16 => DIAG_SCAN_4X4[index], // 16x16 TU is a 4x4 grid of CGs
        32 => DIAG_SCAN_8X8[index], // 32x32 TU is an 8x8 grid of CGs
        _ => panic!("Invalid Transform Unit size: {}", tu_size)
    }
}

/// Translates a 2D (X, Y) coordinate into a 1D index.
/// Used specifically to find where we should start our reverse scan loop!
pub fn get_subblock_index(cg_x: usize, cg_y: usize, tu_size: usize) -> usize {
    let target = (cg_x, cg_y);

    match tu_size {
        4 => 0,
        8 => DIAG_SCAN_2X2.iter().position(|&p| p == target).unwrap_or(0),
        16 => DIAG_SCAN_4X4.iter().position(|&p| p == target).unwrap_or(0),
        32 => DIAG_SCAN_8X8.iter().position(|&p| p == target).unwrap_or(0),
        _ => panic!("Invalid Transform Unit size: {}", tu_size)
    }
}
