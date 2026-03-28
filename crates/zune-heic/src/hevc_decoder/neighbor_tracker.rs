use crate::hevc_decoder::DEBUG_MORE;
use crate::debug_more;

#[derive(Clone, Copy, Debug)]
pub struct BlockState {
    pub available:       bool, // False if off-screen or not yet decoded
    pub skip_flag:       bool,
    pub cqt_depth:       u8,   // Depth at which this 8x8 was decided
    pub is_intra:        bool,
    pub intra_mode_luma: u8,   // 0-34
    pub qp:              i8,
}

impl Default for BlockState {
    fn default() -> Self {
        Self {
            available: false,
            skip_flag: false,
            cqt_depth: 0,
            is_intra: false,
            intra_mode_luma: 1, // Default to DC
            qp: 0,
        }
    }
}

pub struct NeighborTracker {
    /// Full frame map stored in 8x8 units.
    pub blocks: Vec<BlockState>,
    pub width_8x8: usize,
    pub height_8x8: usize,
}

impl NeighborTracker {
    /// Initializes a tracker based on the image dimensions in pixels.
    pub fn new(pic_width: usize, pic_height: usize) -> Self {
        let width_8x8 = (pic_width + 7) / 8;
        let height_8x8 = (pic_height + 7) / 8;

        Self {
            blocks: vec![BlockState::default(); width_8x8 * height_8x8],
            width_8x8,
            height_8x8,
        }
    }

    /// Boundary-safe helper to fetch a state at pixel coordinates.
    pub fn get_state(&self, x: usize, y: usize) -> BlockState {
        let gx = x / 8;
        let gy = y / 8;
        if gx < self.width_8x8 && gy < self.height_8x8 {
            self.blocks[gy * self.width_8x8 + gx]
        } else {
            BlockState::default() // available = false
        }
    }

    /// MISSING FUNCTION 1: update_block
    /// Updates all 8x8 units covered by a block of 'size' (e.g., 32, 16, 8).
    pub fn update_block(&mut self, x: usize, y: usize, size: usize, state: BlockState) {
        let gx_start = x / 8;
        let gy_start = y / 8;
        let units = (size / 8).max(1);

        for dy in 0..units {
            for dx in 0..units {
                let gx = gx_start + dx;
                let gy = gy_start + dy;
                if gx < self.width_8x8 && gy < self.height_8x8 {
                    let idx = gy * self.width_8x8 + gx;
                    self.blocks[idx] = state;
                    self.blocks[idx].available = true; // Mark as decoded
                }
            }
        }
    }

    /// MISSING FUNCTION 2: get_split_ctx
    /// Derived from Spec §9.3.4.2.2. Returns 0, 1, or 2 based on neighbors.
    pub fn get_split_ctx(&self, x: usize, y: usize, current_depth: u8) -> usize {
        // Neighbor Left (one pixel to the left)
        let left = if x > 0 { self.get_state(x - 1, y) } else { BlockState::default() };
        // Neighbor Above (one pixel above)
        let above = if y > 0 { self.get_state(x, y - 1) } else { BlockState::default() };

        let mut ctx_inc = 0;
        if left.available && left.cqt_depth > current_depth {
            ctx_inc += 1;
        }
        if above.available && above.cqt_depth > current_depth {
            ctx_inc += 1;
        }

        ctx_inc
    }

    /// MISSING FUNCTION 3: update_qp
    /// Used when a QP delta is decoded to refresh the area's quantization state.
    pub fn update_qp(&mut self, x: usize, y: usize, size: usize, qp: i8) {
        let gx_start = x / 8;
        let gy_start = y / 8;
        let units = (size / 8).max(1);

        for dy in 0..units {
            for dx in 0..units {
                let gx = gx_start + dx;
                let gy = gy_start + dy;
                if gx < self.width_8x8 && gy < self.height_8x8 {
                    let idx = gy * self.width_8x8 + gx;
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
        if left.available && left.skip_flag { ctx_inc += 1; }
        if above.available && above.skip_flag { ctx_inc += 1; }
        ctx_inc
    }

    pub fn derive_mpms(&self, x: usize, y: usize) -> [u8; 3] {
        let left = if x > 0 { self.get_state(x - 1, y) } else { BlockState::default() };
        let above = if y > 0 { self.get_state(x, y - 1) } else { BlockState::default() };

        let mode_l = if left.available && left.is_intra { left.intra_mode_luma } else { 1 };
        let mode_a = if above.available && above.is_intra { above.intra_mode_luma } else { 1 };

        let mut mpm = [0u8; 3];

        if mode_l == mode_a {
            if mode_l < 2 { // Planar (0) or DC (1)
                mpm = [0, 1, 26]; // 26 is Vertical
            } else {
                // Angular Mode Wrap-around logic (Spec 8.4.2)
                mpm[0] = mode_l;
                // Mode-2 shift + 29 (which is -3) then +2 offset back
                mpm[1] = 2 + ((mode_l - 2 + 29) % 32);
                // Mode-2 shift + 1 then +2 offset back
                mpm[2] = 2 + ((mode_l - 2 + 1) % 32);
            }
        } else {
            mpm[0] = mode_l;
            mpm[1] = mode_a;
            if mode_l != 0 && mode_a != 0 { mpm[2] = 0; }
            else if mode_l != 1 && mode_a != 1 { mpm[2] = 1; }
            else { mpm[2] = 26; }
        }
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
    /// Sets the intra prediction mode for a specific area.
    /// x0, y0: Top-left pixel coordinates of the Prediction Block (PB).
    /// pb_size: Size of the PB in pixels (e.g., 32, 16, 8, or 4).
    /// mode: The decoded intra luma mode (0-34).
    pub fn set_intra_mode(&mut self, x0: usize, y0: usize, pb_size: usize, mode: u8) {
        let gx_start = x0 / 8;
        let gy_start = y0 / 8;

        // Determine how many 8x8 units this block covers.
        // For sizes 32, 16, 8: coverage is 4, 2, 1 units.
        // For size 4 (Intra NxN): .max(1) ensures we still update the containing 8x8 cell.
        let units = (pb_size / 8).max(1);

        debug_more!(
            "Tracker: Setting Intra Mode {} at [{}, {}] size {}",
            mode, x0, y0, pb_size
        );

        for dy in 0..units {
            for dx in 0..units {
                let gx = gx_start + dx;
                let gy = gy_start + dy;

                if gx < self.width_8x8 && gy < self.height_8x8 {
                    let idx = gy * self.width_8x8 + gx;
                    let state = &mut self.blocks[idx];

                    state.is_intra = true;
                    state.intra_mode_luma = mode;
                    state.available = true; // Mark this area as decoded and available for neighbors
                }
            }
        }
    }
}