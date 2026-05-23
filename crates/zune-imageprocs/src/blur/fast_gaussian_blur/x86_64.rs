#![cfg(target_arch = "x86_64")]
use crate::blur::fast_gaussian_blur::RING_SIZE;
use crate::utils::as_mut_array;
use core::arch::x86_64::*;
use zune_image::planar_regions::PlanarRegionOut;
#[target_feature(enable = "sse2")]
pub unsafe fn horizontal_blur_gaussian_inner_u8_sse(
    in_rows: &[&[u8]; 4], ring_buffer: &mut [[i32; 4]; RING_SIZE], out_rows: &mut [&mut [u8]; 4],
    width: usize, radius: usize,
) {
    let area_i32 = radius.pow(2) as i32;
    let initial_sum = area_i32 >> 1;

    // Initialization
    let mut diffs = _mm_setzero_si128();
    let mut sums = _mm_set1_epi32(initial_sum);

    // Magic multiplier for exact 32-bit division
    let magic_multiplier = (1u64 << 32).div_ceil(area_i32 as u64);
    let magic_vec = _mm_set1_epi32(magic_multiplier as i32);

    ring_buffer.fill([0; 4]);

    for i in 0..4 {
        assert!(in_rows[i].len() >= width);
        assert!(out_rows[i].len() >= width);
    }

    let start_x = -radius.cast_signed() * 2;
    let width_isize = width.cast_signed();
    let radius_isize = radius.cast_signed();

    let mut tmp_buffer: [i32; 4] = [0; 4];
    for x in start_x..width_isize {
        let next_x = (x + radius_isize).clamp(0, width_isize - 1) as usize;
        let ring_idx = ((x + radius_isize) as usize) % RING_SIZE;

        if x >= 0 {
            let ux = x as usize;
            let trail_idx_1 = ((x - radius_isize) as usize) % RING_SIZE;
            let trail_idx_2 = ux % RING_SIZE;

            let a_stored = _mm_loadu_si128(ring_buffer[trail_idx_1].as_ptr() as *const __m128i);
            let d_stored = _mm_loadu_si128(ring_buffer[trail_idx_2].as_ptr() as *const __m128i);

            // --- The SSE2 Exact Division Trick ---
            {
                // 1. Multiply the EVEN lanes (0 and 2): u32 * u32 -> u64
                let prod_even = _mm_mul_epu32(sums, magic_vec);

                // 2. Shift sums right by 4 bytes to put ODD lanes into EVEN lanes, then multiply
                let sums_odd = _mm_srli_si128::<4>(sums);
                let prod_odd = _mm_mul_epu32(sums_odd, magic_vec);

                // 3. Shift the 64-bit products right by 32 bits to get the integer division result
                // This leaves the result in the lower 32-bits of each 64-bit lane.
                let res_even = _mm_srli_epi64::<32>(prod_even);
                let res_odd = _mm_srli_epi64::<32>(prod_odd);

                // 4. Shift the odd results left by 4 bytes so they sit in lanes 1 and 3
                let res_odd_shifted = _mm_slli_si128::<4>(res_odd);

                // 5. Bitwise OR them together to re-pack into [Lane0, Lane1, Lane2, Lane3]
                let blurred_val_int = _mm_or_si128(res_even, res_odd_shifted);

                _mm_storeu_si128(tmp_buffer.as_mut_ptr() as *mut __m128i, blurred_val_int);
            }

            // Store to output
            out_rows[0][ux] = tmp_buffer[0] as u8;
            out_rows[1][ux] = tmp_buffer[1] as u8;
            out_rows[2][ux] = tmp_buffer[2] as u8;
            out_rows[3][ux] = tmp_buffer[3] as u8;

            // Load next pixel
            let pixel_val_array = [
                i32::from(in_rows[0][next_x]),
                i32::from(in_rows[1][next_x]),
                i32::from(in_rows[2][next_x]),
                i32::from(in_rows[3][next_x]),
            ];
            let pixel_val = _mm_loadu_si128(pixel_val_array.as_ptr() as *const __m128i);

            // ring_buffer[ring_idx] = pixel_val
            _mm_storeu_si128(
                ring_buffer[ring_idx].as_mut_ptr() as *mut __m128i,
                pixel_val,
            );

            // Calculate: a_stored - (d_stored * 2) + pixel_val
            // Note: d_stored * 2 is just a left shift by 1.
            let left = {
                let d_new = _mm_slli_epi32::<1>(d_stored);
                let a_s = _mm_sub_epi32(a_stored, d_new);
                _mm_add_epi32(a_s, pixel_val)
            };

            diffs = _mm_add_epi32(left, diffs);
            sums = _mm_add_epi32(diffs, sums);
        } else if x + radius_isize >= 0 {
            let trail_idx = (x as usize) % RING_SIZE;
            let stored = _mm_loadu_si128(ring_buffer[trail_idx].as_ptr() as *const __m128i);

            let pixel_val_array = [
                i32::from(in_rows[0][next_x]),
                i32::from(in_rows[1][next_x]),
                i32::from(in_rows[2][next_x]),
                i32::from(in_rows[3][next_x]),
            ];
            let pixel_val = _mm_loadu_si128(pixel_val_array.as_ptr() as *const __m128i);

            _mm_storeu_si128(
                ring_buffer[ring_idx].as_mut_ptr() as *mut __m128i,
                pixel_val,
            );

            let d_new = _mm_slli_epi32::<1>(stored); // stored * 2
            diffs = _mm_add_epi32(diffs, _mm_sub_epi32(pixel_val, d_new));
            sums = _mm_add_epi32(diffs, sums);
        } else {
            let pixel_val_array = [
                i32::from(in_rows[0][next_x]),
                i32::from(in_rows[1][next_x]),
                i32::from(in_rows[2][next_x]),
                i32::from(in_rows[3][next_x]),
            ];
            let pixel_val = _mm_loadu_si128(pixel_val_array.as_ptr() as *const __m128i);

            _mm_storeu_si128(
                ring_buffer[ring_idx].as_mut_ptr() as *mut __m128i,
                pixel_val,
            );

            diffs = _mm_add_epi32(diffs, pixel_val);
            sums = _mm_add_epi32(diffs, sums);
        }
    }
}

#[target_feature(enable = "avx2")]
pub unsafe fn vertical_blur_region_u8_avx2(region: &mut PlanarRegionOut<'_, u8>, radius: usize) {
    let width = region.width;
    let current_height = region.height;
    let y_offset = region.y_offset;

    if current_height == 0 || radius == 0 {
        return;
    }

    let full_height = region.src_channels[0].len() / width;

    const RING_SIZE: usize = 1024;
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
    let mut ring_buffer_vec = vec![sim_zero; RING_SIZE];
    let ring_buffer: &mut [_; RING_SIZE] = as_mut_array(ring_buffer_vec.as_mut_slice()).unwrap();

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        let full_blocks = width / BLOCK;

        // --- Vectorized Columns ---
        for block in 0..full_blocks {
            let x_base = block * BLOCK;

            let mut diffs = sim_zero;
            let mut summs = sim_init;
            ring_buffer.fill(sim_zero);

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

                    let trail_idx_1 = ((y - radius_isize) as usize) % RING_SIZE;
                    let trail_idx_2 = uy % RING_SIZE;

                    let a = ring_buffer[trail_idx_1];
                    let d = ring_buffer[trail_idx_2];

                    let d_x2 = _mm256_add_epi32(d, d);
                    let sub_a_d = _mm256_sub_epi32(a, d_x2);
                    diffs = _mm256_add_epi32(diffs, sub_a_d);
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) % RING_SIZE;
                    let stored = ring_buffer[trail_idx];
                    let stored_x2 = _mm256_add_epi32(stored, stored);
                    diffs = _mm256_sub_epi32(diffs, stored_x2);
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) % RING_SIZE;
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
                    let trail_idx = y as usize % RING_SIZE;
                    let stored = ring_buffer_tail[trail_idx];
                    for col in 0..tail_len {
                        diffs[col] -= stored[col] * 2;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) % RING_SIZE;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blur::fast_gaussian_blur::{fast_gaussian_inner_nx, vertical_blur_region_fast_out};
    use nanorand::{Rng, WyRand};
    use zune_image::planar_regions::PlanarRegionOut;

    #[test]
    fn test_hardware_simd_vs_generic() {
        let mut rng = WyRand::new();

        let test_cases = [
            (8, 20, 3, 0),
            (13, 20, 5, 0),
            (3, 10, 2, 0),
            (32, 50, 15, 10),
            (17, 100, 25, 20),
        ];

        for (width, full_height, radius, y_offset) in test_cases {
            let current_height = full_height - y_offset;
            let full_pixels = width * full_height;
            let region_pixels = width * current_height;

            let mut src = vec![0u8; full_pixels];
            rng.fill(&mut src);

            let mut dest_simd = vec![0u8; region_pixels];
            let mut dest_generic = vec![0u8; region_pixels];

            let mut dest_channels_simd = [&mut dest_simd[..]];
            let mut region_simd = PlanarRegionOut {
                y_offset,
                height: current_height,
                width,
                src_channels: &[&src[..]],
                dest_channels: &mut dest_channels_simd,
            };

            let mut dest_channels_generic = [&mut dest_generic[..]];
            let mut region_generic = PlanarRegionOut {
                y_offset,
                height: current_height,
                width,
                src_channels: &[&src[..]],
                dest_channels: &mut dest_channels_generic,
            };

            // Calculate baseline generically
            vertical_blur_region_fast_out::<u8>(&mut region_generic, radius);

            // Execute specialized SIMD if supported by the host
            let mut simd_executed = false;

            #[cfg(target_arch = "x86_64")]
            {
                if is_x86_feature_detected!("avx2") {
                    unsafe {
                        vertical_blur_region_u8_avx2(&mut region_simd, radius);
                    }
                    simd_executed = true;
                }
            }

            if simd_executed {
                for y in 0..current_height {
                    for x in 0..width {
                        let idx = y * width + x;
                        let s_val = dest_simd[idx];
                        let g_val = dest_generic[idx];

                        assert_eq!(
                            s_val, g_val,
                            "Mismatch at region x: {}, y: {} (width: {}, radius: {}, offset: {}) | SIMD: {}, Generic: {}",
                            x, y, width, radius, y_offset, s_val, g_val
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_simd_vs_generic_parity() {
        let width = 256;
        let test_radii = [2, 3, 5, 8, 15];
        let mut rng = WyRand::new();

        for radius in test_radii {
            // 1. Generate random input data
            let in_data: [Vec<u8>; 4] = std::array::from_fn(|_| {
                let mut row = vec![0u8; width];
                rng.fill(&mut row);
                row
            });

            let in_rows: [&[u8]; 4] = [&in_data[0], &in_data[1], &in_data[2], &in_data[3]];

            // 2. Setup Generic buffers and execute
            let mut ring_buffer_generic = [[0i32; 4]; RING_SIZE];
            let mut out_data_generic: [Vec<u8>; 4] = std::array::from_fn(|_| vec![0u8; width]);
            let (a, rest) = out_data_generic.split_at_mut(1);
            let (b, rest) = rest.split_at_mut(1);
            let (c, d) = rest.split_at_mut(1);
            let mut out_rows_generic: [&mut [u8]; 4] =
                [a[0].as_mut(), b[0].as_mut(), c[0].as_mut(), d[0].as_mut()];

            fast_gaussian_inner_nx::<u8, 4>(
                &in_rows,
                &mut ring_buffer_generic,
                &mut out_rows_generic,
                width,
                radius,
            );

            // 3. Setup SIMD buffers
            let mut ring_buffer_simd = [[0i32; 4]; RING_SIZE];
            let mut out_data_simd: [Vec<u8>; 4] = std::array::from_fn(|_| vec![0u8; width]);
            // We have to scope the mutable borrows
            let (a, rest) = out_data_simd.split_at_mut(1);
            let (b, rest) = rest.split_at_mut(1);
            let (c, d) = rest.split_at_mut(1);
            let mut out_rows_simd: [&mut [u8]; 4] =
                [a[0].as_mut(), b[0].as_mut(), c[0].as_mut(), d[0].as_mut()];

            // 4. Execute the appropriate SIMD function based on architecture
            unsafe {
                horizontal_blur_gaussian_inner_u8_sse(
                    &in_rows,
                    &mut ring_buffer_simd,
                    &mut out_rows_simd,
                    width,
                    radius,
                );
            }

            // 5. Assert absolute parity
            for i in 0..4 {
                assert_eq!(
                    out_data_simd[i], out_data_generic[i],
                    "Mismatch found in row {} at radius {}!",
                    i, radius
                );
            }
        }
    }
}
