#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn vertical_blur_region_u8_avx2(
    region: &mut PlanarRegionOut<'_, u8>,
    radius: usize,
) {
    let width = region.width;
    let current_height = region.height;
    let y_offset = region.y_offset;

    if current_height == 0 || radius == 0 {
        return;
    }

    let full_height = region.src_channels[0].len() / width;

    const RING_MASK: usize = 1023;
    const BLOCK: usize = 8;

    let area = (radius * radius) as i32;
    let initial_sum = area >> 1;
    let weight_f32 = 1.0 / (area as f32);

    // Initialize AVX2 constants
    let sim_init = _mm256_set1_epi32(initial_sum);
    let sim_zero = _mm256_setzero_si256();
    let sim_weight = _mm256_set1_ps(weight_f32);

    let radius_isize = radius as isize;
    let full_height_isize = full_height as isize;
    let start_y = (y_offset as isize) - (radius_isize * 2);
    let end_y = (y_offset + current_height) as isize;

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        let full_blocks = width / BLOCK;

        // --- Vectorized Columns ---
        for block in 0..full_blocks {
            let x_base = block * BLOCK;

            let mut diffs = sim_zero;
            let mut summs = sim_init;
            let mut ring_buffer = [sim_zero; 1024];

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    if y >= y_offset as isize {
                        let local_y = uy - y_offset;
                        let dest_idx = local_y * width + x_base;

                        // Cast 8x i32 -> 8x f32
                        let float_vec = _mm256_cvtepi32_ps(summs);
                        // Multiply by weight
                        let blurred_f32 = _mm256_mul_ps(float_vec, sim_weight);
                        // Truncate back to 8x i32 (matching integer division behavior)
                        let blurred_i32 = _mm256_cvttps_epi32(blurred_f32);

                        // Pack 256-bit (8x i32) down to 8x u8 for memory writing.
                        // 1. Extract the lower and upper 128-bit lanes
                        let lo_128 = _mm256_castsi256_si128(blurred_i32);
                        let hi_128 = _mm256_extracti128_si256(blurred_i32, 1);
                        // 2. Pack 8x i32 into 8x u16 (SSE4.1)
                        let packed16 = _mm_packus_epi32(lo_128, hi_128);
                        // 3. Pack 8x u16 into 16x u8 (SSE2)
                        let packed8 = _mm_packus_epi16(packed16, packed16);

                        // 4. Store exactly the lower 64 bits (8 bytes) to memory
                        _mm_storel_epi64(
                            dest_channel.as_mut_ptr().add(dest_idx) as *mut __m128i,
                            packed8,
                        );
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) & RING_MASK;
                    let trail_idx_2 = uy & RING_MASK;

                    let a = ring_buffer[trail_idx_1];
                    let d = ring_buffer[trail_idx_2];

                    let d_x2 = _mm256_add_epi32(d, d);
                    let sub_a_d = _mm256_sub_epi32(a, d_x2);
                    diffs = _mm256_add_epi32(diffs, sub_a_d);

                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) & RING_MASK;
                    let stored = ring_buffer[trail_idx];
                    let stored_x2 = _mm256_add_epi32(stored, stored);
                    diffs = _mm256_sub_epi32(diffs, stored_x2);
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) & RING_MASK;
                let src_idx = next_y * width + x_base;

                // Load 8 bytes into a 128-bit register
                let bytes = _mm_loadl_epi64(src_channel.as_ptr().add(src_idx) as *const __m128i);

                // Zero-extend the 8x u8 into 8x i32 inside a 256-bit register
                let pixels_i32 = _mm256_cvtepu8_epi32(bytes);

                ring_buffer[ring_idx] = pixels_i32;

                diffs = _mm256_add_epi32(diffs, pixels_i32);
                summs = _mm256_add_epi32(summs, diffs);
            }
        }

        // --- Scalar Tail (Remaining columns < BLOCK) ---
        let tail_start = full_blocks * BLOCK;
        let tail_len = width - tail_start;

        if tail_len > 0 {
            let mut diffs = [0i32; BLOCK];
            let mut summs = [initial_sum; BLOCK];
            let mut ring_buffer_tail = [[0i32; BLOCK]; 1024];

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    if y >= y_offset as isize {
                        let local_y = uy - y_offset;
                        let dest_base = local_y * width + tail_start;
                        for col in 0..tail_len {
                            dest_channel[dest_base + col] = (summs[col] / area) as u8;
                        }
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) & RING_MASK;
                    let trail_idx_2 = uy & RING_MASK;
                    let a = ring_buffer_tail[trail_idx_1];
                    let d = ring_buffer_tail[trail_idx_2];
                    for col in 0..tail_len {
                        diffs[col] += a[col] - (d[col] * 2);
                    }
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) & RING_MASK;
                    let stored = ring_buffer_tail[trail_idx];
                    for col in 0..tail_len {
                        diffs[col] -= stored[col] * 2;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) & RING_MASK;
                let src_base = next_y * width + tail_start;

                for col in 0..tail_len {
                    let pixel_val = src_channel[src_base + col] as i32;
                    ring_buffer_tail[ring_idx][col] = pixel_val;
                    diffs[col] += pixel_val;
                    summs[col] += diffs[col];
                }
            }
        }
    }
}