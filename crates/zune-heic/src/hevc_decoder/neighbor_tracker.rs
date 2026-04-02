use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::constants::PartMode;

#[derive(Clone, Copy, Debug)]
pub struct BlockState {
    pub pred_mode:         PredMode,
    pub part_mode:         PartMode,
    pub slice_id:          u16, // To check if neighbors are in the same slice
    pub available:         bool, // False if off-screen or not yet decoded
    pub skip_flag:         bool,
    pub cqt_depth:         u8, // Depth at which this 8x8 was decided
    pub is_intra:          bool,
    pub intra_mode_luma:   u8,   // 0-34
    pub intra_mode_chroma: u8,   // 0-34 (The mapped direction)
    pub is_chroma_dm:      bool, // True if syntax element was 4
    pub qp:                i8,
    pub has_nonzero_coeff: bool
}

impl Default for BlockState {
    fn default() -> Self {
        Self {
            pred_mode:         PredMode::ModeInter,
            part_mode:         PartMode::Part2Nx2N,
            available:         false,
            skip_flag:         false,
            cqt_depth:         0,
            is_intra:          false,
            intra_mode_luma:   1, // Default to DC
            qp:                0,
            slice_id:          0,
            has_nonzero_coeff: false,
            intra_mode_chroma: 1, // Default to DC
            is_chroma_dm:      false
        }
    }
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PredMode {
    ModeIntra,
    ModeInter,
    ModeSkip // Skip is often treated as a subset of Inter
}
pub struct NeighborTracker {
    /// Full frame map stored in 8x8 units.
    pub blocks:          Vec<BlockState>,
    pub height_in_units: usize,
    pub width_in_units:  usize,
    pub log2_unit_size:  u8 // Usually 2 for 4x4 units, or 3 for 8x8 units
}

impl NeighborTracker {
    /// Initializes a tracker based on the image dimensions in pixels.
    pub fn new(pic_width: usize, pic_height: usize, log2_unit_size: u8) -> Self {
        // Since we are using 8x8 units:
        //let log2_unit_size = 3;
        let width_in_units = pic_width.div_ceil(1 << log2_unit_size);
        let height_in_units = pic_height.div_ceil(1 << log2_unit_size);

        Self {
            blocks: vec![BlockState::default(); width_in_units * height_in_units],
            width_in_units,
            height_in_units,
            log2_unit_size
        }
    }

    /// MISSING FUNCTION 1: update_block
    /// Updates all 8x8 units covered by a block of 'size' (e.g., 32, 16, 8).
    pub fn update_block(&mut self, x: usize, y: usize, size: usize, state: BlockState) {
        let gx_start = x >> self.log2_unit_size;
        let gy_start = y >> self.log2_unit_size;
        let units = (size >> self.log2_unit_size).max(1);

        for dy in 0..units {
            for dx in 0..units {
                let gx = gx_start + dx;
                let gy = gy_start + dy;
                if gx < self.width_in_units && gy < self.height_in_units {
                    let idx = gy * self.width_in_units + gx;
                    self.blocks[idx] = state;
                    self.blocks[idx].available = true; // Mark as decoded
                }
            }
        }
    }
    pub fn update_block_depth(&mut self, x: usize, y: usize, size: usize, ct_depth: u8) {
        let gx_start = x >> self.log2_unit_size;
        let gy_start = y >> self.log2_unit_size;
        let units = (size >> self.log2_unit_size).max(1);

        for dy in 0..units {
            for dx in 0..units {
                let gx = gx_start + dx;
                let gy = gy_start + dy;
                if gx < self.width_in_units && gy < self.height_in_units {
                    let idx = gy * self.width_in_units + gx;
                    self.blocks[idx].cqt_depth = ct_depth; // Mark as decoded
                }
            }
        }
    }

    /// Derived from Spec §9.3.4.2.2. Returns 0, 1, or 2 based on neighbors.
    pub fn get_split_ctx(&self, x: usize, y: usize, current_depth: u8, current_slice_id: u16) -> usize {
        let mut cond_l = 0;
        let mut cond_a = 0;

        if x > 0 {
            let left = self.get_state(x - 1, y);
            // Neighbor is only "Available" if it's in the same slice segment
            if left.available && left.slice_id == current_slice_id {
                if left.cqt_depth > current_depth {
                    cond_l = 1;
                }
            }
        }

        if y > 0 {
            let above = self.get_state(x, y - 1);
            if above.available && above.slice_id == current_slice_id {
                if above.cqt_depth > current_depth {
                    cond_a = 1;
                }
            }
        }

        cond_l + cond_a
    }

    /// Used when a QP delta is decoded to refresh the area's quantization state.
    pub fn update_qp(&mut self, x: usize, y: usize, size: usize, qp: i8) {
        let gx_start = x >> self.log2_unit_size;
        let gy_start = y >> self.log2_unit_size;
        let units = (size >> self.log2_unit_size).max(1);

        for dy in 0..units {
            for dx in 0..units {
                let gx = gx_start + dx;
                let gy = gy_start + dy;
                if gx < self.width_in_units && gy < self.height_in_units {
                    let idx = gy * self.width_in_units + gx;
                    self.blocks[idx].qp = qp;
                    self.blocks[idx].available = true;
                }
            }
        }
    }

    // --- Additional Required Logic ---

    pub fn derive_skip_context(&self, x: usize, y: usize) -> usize {
        let left = if x > 0 { self.get_state(x - 1, y) } else { BlockState::default() };
        let above = if y > 0 { self.get_state(x, y - 1) } else { BlockState::default() };

        let mut ctx_inc = 0;
        if left.available && left.skip_flag {
            ctx_inc += 1;
        }
        if above.available && above.skip_flag {
            ctx_inc += 1;
        }
        ctx_inc
    }

    pub fn derive_mpms(&self, x: usize, y: usize) -> [u8; 3] {
        debug_more!("derive_mpms called with {} and {}", x, y);
        let left = if x > 0 { self.get_state(x - 1, y) } else { BlockState::default() };
        let above = if y > 0 { self.get_state(x, y - 1) } else { BlockState::default() };

        let mode_l = if left.available && left.is_intra { left.intra_mode_luma } else { 1 };
        let mode_a = if above.available && above.is_intra { above.intra_mode_luma } else { 1 };

        let mut mpm = [0u8; 3];

        // 2. Build candidate list (Matching your C code snippet)
        if mode_l == mode_a {
            if mode_l < 2 {
                // Case: Both are Planar or DC
                mpm = [0, 1, 26]; // Planar, DC, Vertical
            } else {
                // Case: Both are the same Angular mode
                mpm[0] = mode_l;
                // Mode - 1 (The '+ 31' is '-1 + 32' to handle the unsigned wrap)
                mpm[1] = 2 + ((mode_l - 2 + 31) % 32);
                // Mode + 1
                mpm[2] = 2 + ((mode_l - 2 + 1) % 32);
            }
        } else {
            // Case: A and B are different
            mpm[0] = mode_l;
            mpm[1] = mode_a;

            if mode_l != 0 && mode_a != 0 {
                mpm[2] = 0; // Filler is Planar
            } else if mode_l != 1 && mode_a != 1 {
                mpm[2] = 1; // Filler is DC
            } else {
                mpm[2] = 26; // Filler is Vertical
            }
        }

        debug_more!(
            "MPM Candidates for [{},{}]: [{}, {}, {}]",
            x,
            y,
            mpm[0],
            mpm[1],
            mpm[2]
        );
        mpm
    }
    pub fn get_qp_left(&self, x: usize, y: usize) -> Option<i8> {
        let s = if x > 0 { self.get_state(x - 1, y) } else { return None };
        if s.available { Some(s.qp) } else { None }
    }

    pub fn get_qp_above(&self, x: usize, y: usize) -> Option<i8> {
        let s = if y > 0 { self.get_state(x, y - 1) } else { return None };
        if s.available { Some(s.qp) } else { None }
    }
}
impl NeighborTracker {
    /// Returns the intra luma mode for the 8x8 block covering pixel (x, y).
    /// This is used by derive_mpms to see what the neighbors chose.
    pub fn get_intra_mode(&self, x: usize, y: usize) -> u8 {
        let ux = x >> self.log2_unit_size;
        let uy = y >> self.log2_unit_size;

        // Safety check for image boundaries
        if ux >= self.width_in_units || uy >= self.height_in_units {
            // If out of bounds, return DC (1) or Planar (0)
            return 1;
        }

        let index = uy * self.width_in_units + ux;
        self.blocks[index].intra_mode_luma
    }

    /// Helper to get the full state for a coordinate (used in your derive_mpms)
    pub fn get_state(&self, x: usize, y: usize) -> BlockState {
        let ux = x >> self.log2_unit_size;
        let uy = y >> self.log2_unit_size;

        if ux >= self.width_in_units || uy >= self.height_in_units {
            // Return a default state with 'available' = false
            return BlockState {
                available: false,
                ..BlockState::default()
            };
        }

        let idx = uy * self.width_in_units + ux;
        self.blocks[idx].clone()
    }
}
impl NeighborTracker {
    /// Sets the intra prediction mode for a specific area.
    /// x0, y0: Top-left pixel coordinates of the Prediction Block (PB).
    /// pb_size: Size of the PB in pixels (e.g., 32, 16, 8, or 4).
    /// mode: The decoded intra luma mode (0-34).
    pub fn set_intra_mode(&mut self, x0: usize, y0: usize, pb_size: usize, mode: u8) {
        let gx_start = x0 >> self.log2_unit_size;
        let gy_start = y0 >> self.log2_unit_size;

        // Determine how many 8x8 units this block covers.
        // For sizes 32, 16, 8: coverage is 4, 2, 1 units.
        // For size 4 (Intra NxN): .max(1) ensures we still update the containing 8x8 cell.
        let units = (pb_size >> self.log2_unit_size).max(1);

        debug_more!(
            "Tracker: Setting Intra Mode {} at [{}, {}] size {}",
            mode,
            x0,
            y0,
            pb_size
        );

        for dy in 0..units {
            for dx in 0..units {
                let gx = gx_start + dx;
                let gy = gy_start + dy;

                if gx < self.width_in_units && gy < self.height_in_units {
                    let idx = gy * self.width_in_units + gx;
                    let state = &mut self.blocks[idx];

                    state.is_intra = true;
                    state.intra_mode_luma = mode;
                    state.available = true; // Mark this area as decoded and available for neighbors
                }
            }
        }
    }
}
impl NeighborTracker {
    pub fn set_pred_mode(&mut self, x: usize, y: usize, log2_blk_size: u8, mode: PredMode) {
        let unit_x = x >> self.log2_unit_size;
        let unit_y = y >> self.log2_unit_size;

        // Calculate how many 8x8 units we need to fill
        // (e.g., a 32x32 block is 4 units wide)
        let width_in_units = 1 << (log2_blk_size - self.log2_unit_size);

        for cy in unit_y..(unit_y + width_in_units) {
            let offset = cy * self.width_in_units;
            for cx in unit_x..(unit_x + width_in_units) {
                // Now setting the actual field inside your blocks vector
                if let Some(block) = self.blocks.get_mut(offset + cx) {
                    block.pred_mode = mode;
                }
            }
        }
    }

    pub fn get_pred_mode(&self, x: usize, y: usize) -> PredMode {
        let ux = x >> self.log2_unit_size;
        let uy = y >> self.log2_unit_size;

        if ux >= self.width_in_units || uy >= self.height_in_units {
            return PredMode::ModeInter; // Treat out-of-bounds as Inter (Unavailable for Intra)
        }

        self.blocks[uy * self.width_in_units + ux].pred_mode
    }
}

impl NeighborTracker {
    /// Sets the PartMode for all units covered by the block
    pub fn set_part_mode(&mut self, x: usize, y: usize, log2_blk_size: u8, mode: PartMode) {
        let unit_x = x >> self.log2_unit_size;
        let unit_y = y >> self.log2_unit_size;
        let width_in_units = 1 << (log2_blk_size - self.log2_unit_size);

        for cy in unit_y..(unit_y + width_in_units) {
            let offset = cy * self.width_in_units;
            for cx in unit_x..(unit_x + width_in_units) {
                if let Some(block) = self.blocks.get_mut(offset + cx) {
                    block.part_mode = mode;
                }
            }
        }
    }

    /// Gets the PartMode of the block containing the pixel (x, y)
    pub fn get_part_mode(&self, x: usize, y: usize) -> PartMode {
        let ux = x >> self.log2_unit_size;
        let uy = y >> self.log2_unit_size;

        // Boundary check
        if ux >= self.width_in_units || uy >= self.height_in_units {
            return PartMode::Part2Nx2N; // Default fallback
        }

        self.blocks[uy * self.width_in_units + ux].part_mode
    }
}

impl NeighborTracker {
    pub fn is_available(
        &self, curr_x: usize, curr_y: usize, neighbor_x: isize, neighbor_y: isize
    ) -> bool {
        // 1. Image Boundary Check
        if neighbor_x < 0 || neighbor_y < 0 {
            return false;
        }

        let nx = neighbor_x as usize;
        let ny = neighbor_y as usize;

        if nx >= self.width_in_units << self.log2_unit_size
            || ny >= self.height_in_units << self.log2_unit_size
        {
            return false;
        }

        // 2. Lookup neighbor data
        let curr_unit = &self.blocks[(curr_y >> self.log2_unit_size) * self.width_in_units + (curr_x >> self.log2_unit_size)];
        let neighbor_unit = &self.blocks[(ny >> self.log2_unit_size) * self.width_in_units + (nx >> self.log2_unit_size)];

        // 3. Slice Boundary Check
        if curr_unit.slice_id != neighbor_unit.slice_id {
            return false;
        }

        // 4. Decoding Order Check
        // A neighbor is only available if it has been marked as 'decoded'
        neighbor_unit.available
    }
}

impl NeighborTracker {
    pub fn set_nonzero_coefficient(&mut self, x: usize, y: usize, log2_trafo_size: u8) {
        let unit_x = x >> self.log2_unit_size;
        let unit_y = y >> self.log2_unit_size;

        // How many 8x8 units wide is this TU?
        let width_in_units = 1 << (log2_trafo_size - self.log2_unit_size);

        for cy in unit_y..(unit_y + width_in_units) {
            let offset = cy * self.width_in_units;
            for cx in unit_x..(unit_x + width_in_units) {
                if let Some(block) = self.blocks.get_mut(offset + cx) {
                    block.has_nonzero_coeff = true;
                }
            }
        }
    }
}

impl NeighborTracker {
    /// Sets the chroma mode using Luma-scale coordinates (Matching libde265)
    /// x0, y0: Top-left LUMA pixel coordinates
    /// log2_blk_size: Luma block size (e.g., 4 for 16x16)
    pub fn set_intra_mode_chroma(
        &mut self, x0: usize, y0: usize, log2_blk_size: u8, mode: u8, is_dm: bool
    ) {
        let gx_start = x0 >> self.log2_unit_size; // Equivalent to x0 / 8
        let gy_start = y0 >> self.log2_unit_size; // Equivalent to y0 / 8

        // How many 8x8 units wide is this Luma block?
        let units = (1 << (log2_blk_size - self.log2_unit_size)).max(1);

        for dy in 0..units {
            for dx in 0..units {
                let gx = gx_start + dx;
                let gy = gy_start + dy;

                if gx < self.width_in_units && gy < self.height_in_units {
                    let idx = gy * self.width_in_units + gx;
                    let state = &mut self.blocks[idx];

                    state.intra_mode_chroma = mode;
                    state.is_chroma_dm = is_dm;
                }
            }
        }
    }
    /// Gets the actual intra direction (0-34) for Chroma at the given LUMA coordinates.
    pub fn get_intra_mode_chroma(&self, x: usize, y: usize) -> u8 {
        let ux = x >> self.log2_unit_size;
        let uy = y >> self.log2_unit_size;

        if ux >= self.width_in_units || uy >= self.height_in_units {
            return 1; // Default to DC
        }

        let state = &self.blocks[uy * self.width_in_units + ux];

        // We return the raw mode (0-34).
        state.intra_mode_chroma
    }
    /// Helper for the 'read_transform_unit' check:
    /// Returns true if the chroma mode at luma position (x,y) was Derived Mode (Mode 4).
    pub fn is_chroma_dm(&self, x: usize, y: usize) -> bool {
        let ux = x >> self.log2_unit_size;
        let uy = y >> self.log2_unit_size;

        if ux >= self.width_in_units || uy >= self.height_in_units {
            return false;
        }

        self.blocks[uy * self.width_in_units + ux].is_chroma_dm
    }
}
