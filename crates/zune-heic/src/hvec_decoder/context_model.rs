/// Represents the decoded state of an 8x8 block.
#[derive(Clone, Copy, Default, Debug)]
pub struct BlockState {
    pub available:       bool, // False if off-screen, in another slice, or not yet decoded
    pub skip_flag:       bool, // Was this block skipped?
    pub cqt_depth:       u8,   // How deeply was the CTU split here? (0 = 64x64, 3 = 8x8)
    pub is_intra:        bool, // Intra (spatial) or Inter (temporal) prediction?
    pub intra_mode_luma: u8    // Stores the 0-34 mode
}

pub struct NeighborTracker {
    /// The row of 8x8 blocks directly above the current CTU row.
    /// Length = picture_width_in_luma_samples / 8
    line_buffer_above: Vec<BlockState>,

    /// The column of 8x8 blocks directly to the left of the current decoding position.
    /// Since the max CTU size is 64x64, we only ever need to track 8 blocks (64/8)
    /// vertically for our immediate left neighbor.
    left_column: [BlockState; 8],

    picture_width_in_8x8: usize
}

impl NeighborTracker {
    pub fn new(pic_width: usize) -> Self {
        let width_8x8 = (pic_width + 7) / 8; // Round up just in case
        Self {
            line_buffer_above:    vec![BlockState::default(); width_8x8],
            left_column:          [BlockState::default(); 8],
            picture_width_in_8x8: width_8x8
        }
    }

    /// Fetches the neighbor above the current x, y coordinate (in pixel space)
    pub fn get_above(&self, x: usize, y: usize) -> BlockState {
        if y == 0 {
            return BlockState::default(); // Top of the screen, not available
        }
        let grid_x = x / 8;
        self.line_buffer_above[grid_x]
    }

    /// Fetches the neighbor to the left of the current x, y coordinate
    pub fn get_left(&self, x: usize, y: usize) -> BlockState {
        if x == 0 {
            return BlockState::default(); // Left edge of the screen, not available
        }
        // y % 64 gives us our vertical position inside the current CTU
        let local_grid_y = (y % 64) / 8;
        self.left_column[local_grid_y]
    }

    /// Called after a CU finishes decoding to update the buffers for the next blocks.
    pub fn update_block(&mut self, x: usize, y: usize, size: usize, state: BlockState) {
        let grid_x_start = x / 8;
        let grid_x_end = (x + size) / 8;
        let local_grid_y_start = (y % 64) / 8;
        let local_grid_y_end = ((y % 64) + size) / 8;

        // Update the line buffer (for the row below us to read eventually)
        for i in grid_x_start..grid_x_end {
            self.line_buffer_above[i] = state;
        }

        // Update the left column (for the block to our right to read)
        for i in local_grid_y_start..local_grid_y_end {
            self.left_column[i] = state;
        }
    }

    /// HEVC Spec §9.3.4.2.2: Derivation process for ctxInc for split_cu_flag
    ///
    /// The context increment depends on whether the neighboring blocks
    /// are deeper (more split) than the current depth.
    pub fn derive_split_cu_context(&self, x: usize, y: usize, current_depth: u8) -> usize {
        let left = self.get_left(x, y);
        let above = self.get_above(x, y);

        let mut ctx_inc = 0;

        // Condition L: If the left block exists AND its depth is greater than ours
        if left.available && left.cqt_depth > current_depth {
            ctx_inc += 1;
        }

        // Condition A: If the above block exists AND its depth is greater than ours
        if above.available && above.cqt_depth > current_depth {
            ctx_inc += 1;
        }

        ctx_inc
    }
    /// Derives the Context Increment (ctxInc) for the `cu_skip_flag`.
    /// Returns 0, 1, or 2.
    pub fn derive_skip_context(&self, x: usize, y: usize) -> usize {
        let left = self.get_left(x, y);
        let above = self.get_above(x, y);

        let mut ctx_inc = 0;

        // Condition L (Left): If the left block is available AND it was skipped
        if left.available && left.skip_flag {
            ctx_inc += 1;
        }

        // Condition A (Above): If the above block is available AND it was skipped
        if above.available && above.skip_flag {
            ctx_inc += 1;
        }

        ctx_inc
    }

    /// HEVC Spec §8.4.2: Derivation process for luma intra prediction mode
    pub fn derive_mpms(&self, x: usize, y: usize) -> [u8; 3] {
        let left = self.get_left(x, y);
        let above = self.get_above(x, y);

        // If a neighbor isn't available or isn't Intra, we assume DC (Mode 1)
        let mode_left = if left.available && left.is_intra { left.intra_mode_luma } else { 1 };
        let mode_above = if above.available && above.is_intra { above.intra_mode_luma } else { 1 };

        let mut mpm = [0u8; 3];

        if mode_left == mode_above {
            if mode_left < 2 {
                // Both are Planar (0) or DC (1)
                mpm[0] = 0; // Planar
                mpm[1] = 1; // DC
                mpm[2] = 26; // Vertical
            } else {
                // Both are the same angular mode
                mpm[0] = mode_left;
                mpm[1] = 2 + ((mode_left + 29) % 32); // Slight angle shift
                mpm[2] = 2 + ((mode_left - 1) % 32); // Slight angle shift opposite
            }
        } else {
            // Neighbors are different. Use both, and fill the 3rd slot with a fallback.
            mpm[0] = mode_left;
            mpm[1] = mode_above;

            if mode_left != 0 && mode_above != 0 {
                mpm[2] = 0; // Planar
            } else if mode_left != 1 && mode_above != 1 {
                mpm[2] = 1; // DC
            } else {
                mpm[2] = 26; // Vertical
            }
        }

        mpm
    }
}

impl NeighborTracker {
    /// Finalized: Returns the full CABAC context index for split_cu_flag (Base 0)
    pub fn get_split_ctx(&self, x: usize, y: usize, current_depth: u8) -> usize {
        let left = self.get_left(x, y);
        let above = self.get_above(x, y);

        let mut ctx_inc = 0;
        if left.available && left.cqt_depth > current_depth { ctx_inc += 1; }
        if above.available && above.cqt_depth > current_depth { ctx_inc += 1; }

        ctx_inc // split_cu_flag uses contexts 0, 1, 2
    }

    /// Finalized: Updates the tracker state for an 8x8-aligned area
    pub fn set_split(&mut self, x: usize, y: usize, size: usize, depth: u8) {
        let grid_x_start = x / 8;
        let grid_x_end = (x + size) / 8;
        let local_y_start = (y % 64) / 8;
        let local_y_end = ((y % 64) + size) / 8;

        let state = BlockState {
            available: true,
            cqt_depth: depth,
            skip_flag: false,
            is_intra: true,
            intra_mode_luma: 1, // Default DC
        };

        for i in grid_x_start..grid_x_end {
            if i < self.line_buffer_above.len() { self.line_buffer_above[i] = state; }
        }
        for i in local_y_start..local_y_end {
            if i < 8 { self.left_column[i] = state; }
        }
    }
}