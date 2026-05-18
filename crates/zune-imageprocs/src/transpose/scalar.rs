/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use rayon::prelude::*;

/// The L1 cache-friendly block size.
const BLOCK_SIZE: usize = 32;

/// A parallel, cache-oblivious scalar transpose using safe Rayon chunks.
pub fn transpose_scalar<T: Copy + Send + Sync>(
    in_matrix: &[T],
    out_matrix: &mut [T],
    width: usize,  // Source width
    height: usize, // Source height
) {
    if width == 0 || height == 0 {
        return;
    }

    // The destination image has swapped dimensions
    let dest_width = height;
    // Every thread will process a horizontal band of the DESTINATION image.
    // A band consists of BLOCK_SIZE rows (which equals dest_width * BLOCK_SIZE pixels).
    let pixels_per_band = dest_width * BLOCK_SIZE;

    out_matrix
        .par_chunks_mut(pixels_per_band)
        .enumerate()
        .for_each(|(band_idx, dest_band)| {
            let y_dst_start = band_idx * BLOCK_SIZE;
            let band_height = dest_band.len() / dest_width;

            // Process the band in blocks along the x-axis
            for x_block in (0..dest_width).step_by(BLOCK_SIZE) {
                let x_end = (x_block + BLOCK_SIZE).min(dest_width);

                // Note: We put `x` on the outside and `y` on the inside of this block loop.
                // This results in sequential reads from the source matrix, which the CPU
                // prefetcher loves, and strided writes to the destination matrix, which
                // is fine because the 32x32 block fits entirely inside the L1 cache.
                for x_dst in x_block..x_end {
                    let src_row_offset = x_dst * width;

                    for y_local in 0..band_height {
                        let y_dst = y_dst_start + y_local;

                        // x_dst maps to source Y, y_dst maps to source X
                        let src_idx = src_row_offset + y_dst;
                        let dst_idx = y_local * dest_width + x_dst;

                        dest_band[dst_idx] = in_matrix[src_idx];
                    }
                }
            }
        });
}
