#![cfg(target_arch = "aarch64")]
use zune_image::planar_regions::PlanarRegionOut;

use crate::blur::fast_gaussian_blur::RING_SIZE;
use crate::utils::as_mut_array;
use core::arch::aarch64::*;

#[target_feature(enable = "neon")]
pub unsafe fn horizontal_blur_gaussian_inner_u8_neon(
    in_rows: &[&[u8]; 4], ring_buffer: &mut [[i32; 4]; RING_SIZE], out_rows: &mut [&mut [u8]; 4],
    width: usize, radius: usize,
) {
    let area_i32 = radius.pow(2) as i32;
    let initial_sum = area_i32 >> 1;
    let two = vdupq_n_s32(2);

    let mut diffs = vdupq_n_s32(0);
    let mut sums = vdupq_n_s32(initial_sum);
    // Calculate the magic multiplier for exact 32-bit division
    let magic_multiplier = (1u64 << 32).div_ceil(area_i32 as u64);

    // Broadcast to a 64-bit vector (2 lanes of 32-bits) for the multiply long instruction
    let magic_half = vdup_n_u32(magic_multiplier as u32);

    ring_buffer.fill([0; 4]);

    for i in 0..4 {
        assert!(in_rows[i].len() >= width);
        assert!(out_rows[i].len() >= width);
    }

    let start_x = -radius.cast_signed() * 2;
    let width_isize = width.cast_signed();
    let radius_isize = radius.cast_signed();

    let mut tmp_buffer: [i32; 4] = [0; 4];
    let start_x = -radius.cast_signed() * 2;
    let width_isize = width.cast_signed();
    let radius_isize = radius.cast_signed();
    let mut tmp_buffer: [i32; 4] = [0; 4];

    // -------------------------------------------------------------------------
    // Phase 1: Pre-fill (x < -radius)
    // No trail reads, just ring write and sum updates.
    // -------------------------------------------------------------------------
    for x in start_x..-radius_isize {
        // Because x + radius is < 0 here, next_x will always clamp to 0.
        let next_x = 0;
        let ring_idx = ((x + radius_isize) as usize) % RING_SIZE;

        let pixel_val_array = [
            i32::from(in_rows[0][next_x]),
            i32::from(in_rows[1][next_x]),
            i32::from(in_rows[2][next_x]),
            i32::from(in_rows[3][next_x]),
        ];

        let pixel_val = vld1q_s32(pixel_val_array.as_ptr().cast());
        ring_buffer[ring_idx] = pixel_val_array;

        diffs = vaddq_s32(diffs, pixel_val);
        sums = vaddq_s32(diffs, sums);
    }

    // -------------------------------------------------------------------------
    // Phase 2: Ramp-up (-radius <= x < 0)
    // Reads one trail index (stored), updates diff, writes to ring, updates sum.
    // -------------------------------------------------------------------------
    for x in -radius_isize..0 {
        let next_x = (x + radius_isize).clamp(0, width_isize - 1) as usize;
        let ring_idx = ((x + radius_isize) as usize) % RING_SIZE;
        let trail_idx = (x as usize) % RING_SIZE;

        let stored_buf = ring_buffer[trail_idx];
        let stored = vld1q_s32(stored_buf.as_ptr());

        let pixel_val_array = [
            i32::from(in_rows[0][next_x]),
            i32::from(in_rows[1][next_x]),
            i32::from(in_rows[2][next_x]),
            i32::from(in_rows[3][next_x]),
        ];

        let pixel_val = vld1q_s32(pixel_val_array.as_ptr().cast());
        ring_buffer[ring_idx] = pixel_val_array;

        // diffs = diffs + (pixel_val - (stored * two))
        diffs = vaddq_s32(diffs, vsubq_s32(pixel_val, vmulq_s32(stored, two)));
        sums = vaddq_s32(diffs, sums);
    }

    // -------------------------------------------------------------------------
    // Phase 3: Main Body (x >= 0)
    // Full pass: Output generation, 2 trail reads, diff update, sum update.
    // -------------------------------------------------------------------------
    for x in 0..width_isize {
        let next_x = (x + radius_isize).clamp(0, width_isize - 1) as usize;
        let ring_idx = ((x + radius_isize) as usize) % RING_SIZE;

        let ux = x as usize;
        let trail_idx_1 = ((x - radius_isize) as usize) % RING_SIZE;
        let trail_idx_2 = ux % RING_SIZE;

        let a_stored = vld1q_s32(ring_buffer[trail_idx_1].as_ptr());
        let d_stored = vld1q_s32(ring_buffer[trail_idx_2].as_ptr());

        // --- Exact Division Trick ---
        {
            let sums_u32 = vreinterpretq_u32_s32(sums);
            let sums_low = vget_low_u32(sums_u32);
            let sums_high = vget_high_u32(sums_u32);

            let prod_low = vmull_u32(sums_low, magic_half);
            let prod_high = vmull_u32(sums_high, magic_half);

            let result_low = vshrn_n_u64::<32>(prod_low);
            let result_high = vshrn_n_u64::<32>(prod_high);

            let blurred_val_u32 = vcombine_u32(result_low, result_high);
            let blurred_val_int = vreinterpretq_s32_u32(blurred_val_u32);

            vst1q_s32(tmp_buffer.as_mut_ptr().cast(), blurred_val_int);
        }

        // Store in rows
        out_rows[0][ux] = tmp_buffer[0] as u8;
        out_rows[1][ux] = tmp_buffer[1] as u8;
        out_rows[2][ux] = tmp_buffer[2] as u8;
        out_rows[3][ux] = tmp_buffer[3] as u8;

        let pixel_val_array = [
            i32::from(in_rows[0][next_x]),
            i32::from(in_rows[1][next_x]),
            i32::from(in_rows[2][next_x]),
            i32::from(in_rows[3][next_x]),
        ];

        let pixel_val = vld1q_s32(pixel_val_array.as_ptr().cast());
        vst1q_s32(ring_buffer[ring_idx].as_mut_ptr().cast(), pixel_val);

        let left = {
            let d_new = vmulq_s32(d_stored, two);
            let a_s = vsubq_s32(a_stored, d_new);
            vaddq_s32(a_s, pixel_val)
        };

        diffs = vaddq_s32(left, diffs);
        sums = vaddq_s32(diffs, sums);
    }

}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
pub unsafe fn vertical_blur_region_u8_neon(region: &mut PlanarRegionOut<'_, u8>, radius: usize) {
    const RING_SIZE: usize = 1024;
    const BLOCK: usize = 8;

    let width = region.width;
    let current_height = region.height;
    let y_offset = region.y_offset;

    if current_height == 0 || radius == 0 {
        return;
    }

    let full_height = region.src_channels[0].len() / width;

    let area = (radius * radius) as i32;
    let initial_sum = area >> 1;
    let weight_f32 = 1.0 / (area as f32);

    // 1. Initialize our SIMD constants
    let sim_init = vdupq_n_s32(initial_sum);
    let sim_zero = vdupq_n_s32(0);
    let sim_weight = vdupq_n_f32(weight_f32);

    let radius_isize = radius.cast_signed();
    let full_height_isize = full_height.cast_signed();
    let start_y = y_offset.cast_signed() - (radius_isize * 2);
    let end_y = (y_offset + current_height).cast_signed();

    let mut ring_buffer_vec = vec![(sim_zero, sim_zero); 1024];
    let ring_buffer: &mut [_; 1024] = as_mut_array(ring_buffer_vec.as_mut_slice()).unwrap();

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        let full_blocks = width / BLOCK;

        // --- Vectorized Columns ---
        for block in 0..full_blocks {
            let x_base = block * BLOCK;

            let mut diffs_lo = sim_zero;
            let mut diffs_hi = sim_zero;
            let mut summs_lo = sim_init;
            let mut summs_hi = sim_init;
            ring_buffer.fill((sim_zero, sim_zero));

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    // Write to the destination if we have crossed the active region threshold
                    if y >= y_offset as isize {
                        let local_y = uy - y_offset;
                        let dest_idx = local_y * width + x_base;

                        // Division Trick: Convert -> Multiply by reciprocal -> Convert
                        let float_lo = vcvtq_f32_s32(summs_lo);
                        let float_hi = vcvtq_f32_s32(summs_hi);

                        let blurred_f32_lo = vmulq_f32(float_lo, sim_weight);
                        let blurred_f32_hi = vmulq_f32(float_hi, sim_weight);

                        let blurred_i32_lo = vcvtq_s32_f32(blurred_f32_lo);
                        let blurred_i32_hi = vcvtq_s32_f32(blurred_f32_hi);

                        // Narrow down to u16 and saturate
                        let narrowed_u16_lo = vqmovun_s32(blurred_i32_lo);
                        let narrowed_u16_hi = vqmovun_s32(blurred_i32_hi);

                        // Combine into u16x8, then narrow to u8x8 and saturate
                        let combined_u16 = vcombine_u16(narrowed_u16_lo, narrowed_u16_hi);
                        let final_u8x8 = vqmovn_u16(combined_u16);

                        // Store 8 pixels to the mutable destination slice
                        vst1_u8(
                            dest_channel[dest_idx..dest_idx + 8].as_mut_ptr().cast(),
                            final_u8x8,
                        );
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) % RING_SIZE;
                    let trail_idx_2 = uy % RING_SIZE;

                    let (a_lo, a_hi) = ring_buffer[trail_idx_1];
                    let (d_lo, d_hi) = ring_buffer[trail_idx_2];

                    let d_x2_lo = vaddq_s32(d_lo, d_lo);
                    let d_x2_hi = vaddq_s32(d_hi, d_hi);

                    diffs_lo = vaddq_s32(diffs_lo, vsubq_s32(a_lo, d_x2_lo));
                    diffs_hi = vaddq_s32(diffs_hi, vsubq_s32(a_hi, d_x2_hi));
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) % RING_SIZE;
                    let (stored_lo, stored_hi) = ring_buffer[trail_idx];

                    let stored_x2_lo = vaddq_s32(stored_lo, stored_lo);
                    let stored_x2_hi = vaddq_s32(stored_hi, stored_hi);

                    diffs_lo = vsubq_s32(diffs_lo, stored_x2_lo);
                    diffs_hi = vsubq_s32(diffs_hi, stored_x2_hi);
                }

                // Global clamped coordinate for the immutable source reading
                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) % RING_SIZE;
                let src_idx = next_y * width + x_base;

                // Load 8 contiguous u8 bytes
                let pixels_u8x8 = vld1_u8(src_channel.as_ptr().add(src_idx));
                let pixels_u16x8 = vmovl_u8(pixels_u8x8);

                // Split and widen to u32
                let pixels_u32_lo = vmovl_u16(vget_low_u16(pixels_u16x8));
                let pixels_u32_hi = vmovl_u16(vget_high_u16(pixels_u16x8));

                // Reinterpret as signed i32 for accumulation
                let pixels_i32_lo = vreinterpretq_s32_u32(pixels_u32_lo);
                let pixels_i32_hi = vreinterpretq_s32_u32(pixels_u32_hi);

                ring_buffer[ring_idx] = (pixels_i32_lo, pixels_i32_hi);

                diffs_lo = vaddq_s32(diffs_lo, pixels_i32_lo);
                diffs_hi = vaddq_s32(diffs_hi, pixels_i32_hi);

                summs_lo = vaddq_s32(summs_lo, diffs_lo);
                summs_hi = vaddq_s32(summs_hi, diffs_hi);
            }
        }

        // --- Scalar Tail (Remaining columns < BLOCK) ---
        let tail_start = full_blocks * BLOCK;
        let tail_len = width - tail_start;

        if tail_len > 0 {
            let mut diffs = [0i32; BLOCK];
            let mut summs = [initial_sum; BLOCK];
            let mut ring_buffer_tail = vec![[0i32; BLOCK]; 1024];

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

                    let trail_idx_1 = ((y - radius_isize) as usize) % RING_SIZE;
                    let trail_idx_2 = uy % RING_SIZE;
                    let a = ring_buffer_tail[trail_idx_1];
                    let d = ring_buffer_tail[trail_idx_2];

                    for col in 0..tail_len {
                        diffs[col] += a[col] - (d[col] * 2);
                    }
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) % RING_SIZE;
                    let stored = ring_buffer_tail[trail_idx];
                    for col in 0..tail_len {
                        diffs[col] -= stored[col] * 2;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) % RING_SIZE;
                let src_base = next_y * width + tail_start;

                for col in 0..tail_len {
                    let pixel_val = i32::from(src_channel[src_base + col]);
                    ring_buffer_tail[ring_idx][col] = pixel_val;
                    diffs[col] += pixel_val;
                    summs[col] += diffs[col];
                }
            }
        }
    }
}

#[cfg(all(test, target_arch = "aarch64"))]
mod tests {
    use super::*;
    use crate::blur::fast_gaussian_blur::{fast_gaussian_inner_nx, vertical_blur_region_fast_out};
    use nanorand::{Rng, WyRand};
    use zune_image::planar_regions::PlanarRegionOut;

    #[test]
    fn test_neon_vs_generic_vertical_blur() {
        let mut rng = WyRand::new();

        // (width, full_height, radius, y_offset)
        // We test various configurations to hit the 8-lane SIMD blocks,
        // the scalar tail fallbacks, and the region slice offsets.
        let test_cases = [
            (8, 20, 3, 0),     // Perfect block (exactly 8 wide)
            (13, 20, 5, 0),    // Block + Tail (8 + 5)
            (3, 10, 2, 0),     // Only Tail (width < 8)
            (32, 50, 15, 10),  // Larger image with a region offset
            (17, 100, 25, 20), // Large radius, block + tail, with offset
            (120, 200, 10, 0),
        ];

        for (width, full_height, radius, y_offset) in test_cases {
            let current_height = full_height - y_offset;
            let full_pixels = width * full_height;
            let region_pixels = width * current_height;

            // 1. Generate the random full-size source image
            let mut src = vec![0u8; full_pixels];
            rng.fill(&mut src);

            // 2. Allocate separate destination buffers (sized for the specific region, not full image)
            let mut dest_neon = vec![0u8; region_pixels];
            let mut dest_generic = vec![0u8; region_pixels];

            // 3. Create PlanarRegionOut for NEON
            let mut dest_channels_neon = [&mut dest_neon[..]];
            let mut region_neon = PlanarRegionOut {
                y_offset,
                height: current_height,
                width,
                src_channels: &[&src[..]],
                dest_channels: &mut dest_channels_neon,
            };

            // 4. Create PlanarRegionOut for Generic
            let mut dest_channels_generic = [&mut dest_generic[..]];
            let mut region_generic = PlanarRegionOut {
                y_offset,
                height: current_height,
                width,
                src_channels: &[&src[..]],
                dest_channels: &mut dest_channels_generic,
            };

            // 5. Run both implementations
            unsafe {
                vertical_blur_region_u8_neon(&mut region_neon, radius);
            }
            vertical_blur_region_fast_out::<u8>(&mut region_generic, radius);

            // 6. Verify pixel-perfect equality
            for y in 0..current_height {
                for x in 0..width {
                    let idx = y * width + x;
                    let n_val = dest_neon[idx];
                    let g_val = dest_generic[idx];

                    assert_eq!(
                        n_val, g_val,
                        "Mismatch at region x: {x}, y: {y} (width: {width}, radius: {radius}, offset: {y_offset}) | NEON: {n_val}, Generic: {g_val}",
                    );
                }
            }
        }
    }

    #[test]
    fn test_neon_vs_generic_parity() {
        let width = 256;

        // Testing a few different radii (starting at 2 to avoid the area=1 overflow)
        let test_radii = [2, 3, 5, 8, 15];
        let mut rng = WyRand::new();

        for radius in test_radii {
            // 1. Generate random input data using nanorand
            let in_data: [Vec<u8>; 4] = std::array::from_fn(|_| {
                let mut row = vec![0u8; width];
                rng.fill(&mut row);
                row
            });

            // Borrow as slices for the function signature
            let in_rows: [&[u8]; 4] = [&in_data[0], &in_data[1], &in_data[2], &in_data[3]];

            // 2. Setup buffers for the NEON function
            let mut ring_buffer_neon = [[0i32; 4]; RING_SIZE];
            let mut out_data_neon: [Vec<u8>; 4] = std::array::from_fn(|_| vec![0u8; width]);
            // We have to scope the mutable borrows
            let (a, rest) = out_data_neon.split_at_mut(1);
            let (b, rest) = rest.split_at_mut(1);
            let (c, d) = rest.split_at_mut(1);
            let mut out_rows_neon: [&mut [u8]; 4] =
                [a[0].as_mut(), b[0].as_mut(), c[0].as_mut(), d[0].as_mut()];

            // 3. Setup buffers for the Generic function
            let mut ring_buffer_generic = [[0i32; 4]; RING_SIZE];
            let mut out_data_generic: [Vec<u8>; 4] = std::array::from_fn(|_| vec![0u8; width]);
            // We have to scope the mutable borrows
            let (a, rest) = out_data_generic.split_at_mut(1);
            let (b, rest) = rest.split_at_mut(1);
            let (c, d) = rest.split_at_mut(1);
            let mut out_rows_generic: [&mut [u8]; 4] =
                [a[0].as_mut(), b[0].as_mut(), c[0].as_mut(), d[0].as_mut()];

            // 4. Execute both functions
            unsafe {
                horizontal_blur_gaussian_inner_u8_neon(
                    &in_rows,
                    &mut ring_buffer_neon,
                    &mut out_rows_neon,
                    width,
                    radius,
                );
            }

            fast_gaussian_inner_nx::<u8, 4>(
                &in_rows,
                &mut ring_buffer_generic,
                &mut out_rows_generic,
                width,
                radius,
            );

            // 5. Assert absolute parity
            for i in 0..4 {
                assert_eq!(
                    out_data_neon[i], out_data_generic[i],
                    "Mismatch found in row {} at radius {}!",
                    i, radius
                );
            }
        }
    }
}
