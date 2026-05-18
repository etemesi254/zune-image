#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;
#[cfg(target_arch = "aarch64")]
use rayon::prelude::*;

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn transpose_8_by_8_neon_u8(
    in_matrix: &[u8], out: &mut [u8], in_stride: usize, out_stride: usize,
) {
    // Load 8 rows of 8 bytes (64-bit vectors)
    let r0 = vld1_u8(in_matrix.as_ptr());
    let r1 = vld1_u8(in_matrix.as_ptr().add(in_stride));
    let r2 = vld1_u8(in_matrix.as_ptr().add(in_stride * 2));
    let r3 = vld1_u8(in_matrix.as_ptr().add(in_stride * 3));
    let r4 = vld1_u8(in_matrix.as_ptr().add(in_stride * 4));
    let r5 = vld1_u8(in_matrix.as_ptr().add(in_stride * 5));
    let r6 = vld1_u8(in_matrix.as_ptr().add(in_stride * 6));
    let r7 = vld1_u8(in_matrix.as_ptr().add(in_stride * 7));

    // Pass 1: Zip 8-bit elements
    let z01_l = vzip1_u8(r0, r1);
    let z01_h = vzip2_u8(r0, r1);
    let z23_l = vzip1_u8(r2, r3);
    let z23_h = vzip2_u8(r2, r3);
    let z45_l = vzip1_u8(r4, r5);
    let z45_h = vzip2_u8(r4, r5);
    let z67_l = vzip1_u8(r6, r7);
    let z67_h = vzip2_u8(r6, r7);

    // Pass 2: Zip 16-bit elements
    let y0123_0 = vreinterpret_u8_u16(vzip1_u16(vreinterpret_u16_u8(z01_l), vreinterpret_u16_u8(z23_l)));
    let y0123_1 = vreinterpret_u8_u16(vzip2_u16(vreinterpret_u16_u8(z01_l), vreinterpret_u16_u8(z23_l)));
    let y0123_2 = vreinterpret_u8_u16(vzip1_u16(vreinterpret_u16_u8(z01_h), vreinterpret_u16_u8(z23_h)));
    let y0123_3 = vreinterpret_u8_u16(vzip2_u16(vreinterpret_u16_u8(z01_h), vreinterpret_u16_u8(z23_h)));

    let y4567_0 = vreinterpret_u8_u16(vzip1_u16(vreinterpret_u16_u8(z45_l), vreinterpret_u16_u8(z67_l)));
    let y4567_1 = vreinterpret_u8_u16(vzip2_u16(vreinterpret_u16_u8(z45_l), vreinterpret_u16_u8(z67_l)));
    let y4567_2 = vreinterpret_u8_u16(vzip1_u16(vreinterpret_u16_u8(z45_h), vreinterpret_u16_u8(z67_h)));
    let y4567_3 = vreinterpret_u8_u16(vzip2_u16(vreinterpret_u16_u8(z45_h), vreinterpret_u16_u8(z67_h)));

    // Pass 3: Zip 32-bit elements
    let out0 = vreinterpret_u8_u32(vzip1_u32(vreinterpret_u32_u8(y0123_0), vreinterpret_u32_u8(y4567_0)));
    let out1 = vreinterpret_u8_u32(vzip2_u32(vreinterpret_u32_u8(y0123_0), vreinterpret_u32_u8(y4567_0)));
    let out2 = vreinterpret_u8_u32(vzip1_u32(vreinterpret_u32_u8(y0123_1), vreinterpret_u32_u8(y4567_1)));
    let out3 = vreinterpret_u8_u32(vzip2_u32(vreinterpret_u32_u8(y0123_1), vreinterpret_u32_u8(y4567_1)));
    let out4 = vreinterpret_u8_u32(vzip1_u32(vreinterpret_u32_u8(y0123_2), vreinterpret_u32_u8(y4567_2)));
    let out5 = vreinterpret_u8_u32(vzip2_u32(vreinterpret_u32_u8(y0123_2), vreinterpret_u32_u8(y4567_2)));
    let out6 = vreinterpret_u8_u32(vzip1_u32(vreinterpret_u32_u8(y0123_3), vreinterpret_u32_u8(y4567_3)));
    let out7 = vreinterpret_u8_u32(vzip2_u32(vreinterpret_u32_u8(y0123_3), vreinterpret_u32_u8(y4567_3)));

    // Store transposed 8x8 matrix
    vst1_u8(out.as_mut_ptr(), out0);
    vst1_u8(out.as_mut_ptr().add(out_stride), out1);
    vst1_u8(out.as_mut_ptr().add(out_stride * 2), out2);
    vst1_u8(out.as_mut_ptr().add(out_stride * 3), out3);
    vst1_u8(out.as_mut_ptr().add(out_stride * 4), out4);
    vst1_u8(out.as_mut_ptr().add(out_stride * 5), out5);
    vst1_u8(out.as_mut_ptr().add(out_stride * 6), out6);
    vst1_u8(out.as_mut_ptr().add(out_stride * 7), out7);
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn transpose_8_by_8_neon_u16(
    in_matrix: &[u16], out: &mut [u16], in_stride: usize, out_stride: usize,
) {
    // Load 8 rows of 8 words (128-bit vectors)
    let r0 = vld1q_u16(in_matrix.as_ptr());
    let r1 = vld1q_u16(in_matrix.as_ptr().add(in_stride));
    let r2 = vld1q_u16(in_matrix.as_ptr().add(in_stride * 2));
    let r3 = vld1q_u16(in_matrix.as_ptr().add(in_stride * 3));
    let r4 = vld1q_u16(in_matrix.as_ptr().add(in_stride * 4));
    let r5 = vld1q_u16(in_matrix.as_ptr().add(in_stride * 5));
    let r6 = vld1q_u16(in_matrix.as_ptr().add(in_stride * 6));
    let r7 = vld1q_u16(in_matrix.as_ptr().add(in_stride * 7));

    // Pass 1: Zip 16-bit elements
    let z01_l = vzip1q_u16(r0, r1);
    let z01_h = vzip2q_u16(r0, r1);
    let z23_l = vzip1q_u16(r2, r3);
    let z23_h = vzip2q_u16(r2, r3);
    let z45_l = vzip1q_u16(r4, r5);
    let z45_h = vzip2q_u16(r4, r5);
    let z67_l = vzip1q_u16(r6, r7);
    let z67_h = vzip2q_u16(r6, r7);

    // Pass 2: Zip 32-bit elements
    let y0123_0 = vreinterpretq_u16_u32(vzip1q_u32(vreinterpretq_u32_u16(z01_l), vreinterpretq_u32_u16(z23_l)));
    let y0123_1 = vreinterpretq_u16_u32(vzip2q_u32(vreinterpretq_u32_u16(z01_l), vreinterpretq_u32_u16(z23_l)));
    let y0123_2 = vreinterpretq_u16_u32(vzip1q_u32(vreinterpretq_u32_u16(z01_h), vreinterpretq_u32_u16(z23_h)));
    let y0123_3 = vreinterpretq_u16_u32(vzip2q_u32(vreinterpretq_u32_u16(z01_h), vreinterpretq_u32_u16(z23_h)));

    let y4567_0 = vreinterpretq_u16_u32(vzip1q_u32(vreinterpretq_u32_u16(z45_l), vreinterpretq_u32_u16(z67_l)));
    let y4567_1 = vreinterpretq_u16_u32(vzip2q_u32(vreinterpretq_u32_u16(z45_l), vreinterpretq_u32_u16(z67_l)));
    let y4567_2 = vreinterpretq_u16_u32(vzip1q_u32(vreinterpretq_u32_u16(z45_h), vreinterpretq_u32_u16(z67_h)));
    let y4567_3 = vreinterpretq_u16_u32(vzip2q_u32(vreinterpretq_u32_u16(z45_h), vreinterpretq_u32_u16(z67_h)));

    // Pass 3: Zip 64-bit elements
    let out0 = vreinterpretq_u16_u64(vzip1q_u64(vreinterpretq_u64_u16(y0123_0), vreinterpretq_u64_u16(y4567_0)));
    let out1 = vreinterpretq_u16_u64(vzip2q_u64(vreinterpretq_u64_u16(y0123_0), vreinterpretq_u64_u16(y4567_0)));
    let out2 = vreinterpretq_u16_u64(vzip1q_u64(vreinterpretq_u64_u16(y0123_1), vreinterpretq_u64_u16(y4567_1)));
    let out3 = vreinterpretq_u16_u64(vzip2q_u64(vreinterpretq_u64_u16(y0123_1), vreinterpretq_u64_u16(y4567_1)));
    let out4 = vreinterpretq_u16_u64(vzip1q_u64(vreinterpretq_u64_u16(y0123_2), vreinterpretq_u64_u16(y4567_2)));
    let out5 = vreinterpretq_u16_u64(vzip2q_u64(vreinterpretq_u64_u16(y0123_2), vreinterpretq_u64_u16(y4567_2)));
    let out6 = vreinterpretq_u16_u64(vzip1q_u64(vreinterpretq_u64_u16(y0123_3), vreinterpretq_u64_u16(y4567_3)));
    let out7 = vreinterpretq_u16_u64(vzip2q_u64(vreinterpretq_u64_u16(y0123_3), vreinterpretq_u64_u16(y4567_3)));

    // Store transposed 8x8 matrix
    vst1q_u16(out.as_mut_ptr(), out0);
    vst1q_u16(out.as_mut_ptr().add(out_stride), out1);
    vst1q_u16(out.as_mut_ptr().add(out_stride * 2), out2);
    vst1q_u16(out.as_mut_ptr().add(out_stride * 3), out3);
    vst1q_u16(out.as_mut_ptr().add(out_stride * 4), out4);
    vst1q_u16(out.as_mut_ptr().add(out_stride * 5), out5);
    vst1q_u16(out.as_mut_ptr().add(out_stride * 6), out6);
    vst1q_u16(out.as_mut_ptr().add(out_stride * 7), out7);
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn transpose_neon_float_4x4_inner(
    in_matrix: &[f32], out: &mut [f32], in_stride: usize, out_stride: usize,
) {
    // Load 4 rows of 4 floats (128-bit vectors)
    let r0 = vld1q_f32(in_matrix.as_ptr());
    let r1 = vld1q_f32(in_matrix.as_ptr().add(in_stride));
    let r2 = vld1q_f32(in_matrix.as_ptr().add(in_stride * 2));
    let r3 = vld1q_f32(in_matrix.as_ptr().add(in_stride * 3));

    // Pass 1: Zip 32-bit floats
    let z01_l = vzip1q_f32(r0, r1);
    let z01_h = vzip2q_f32(r0, r1);
    let z23_l = vzip1q_f32(r2, r3);
    let z23_h = vzip2q_f32(r2, r3);

    // Pass 2: Zip 64-bit blocks
    let out0 = vreinterpretq_f32_u64(vzip1q_u64(vreinterpretq_u64_f32(z01_l), vreinterpretq_u64_f32(z23_l)));
    let out1 = vreinterpretq_f32_u64(vzip2q_u64(vreinterpretq_u64_f32(z01_l), vreinterpretq_u64_f32(z23_l)));
    let out2 = vreinterpretq_f32_u64(vzip1q_u64(vreinterpretq_u64_f32(z01_h), vreinterpretq_u64_f32(z23_h)));
    let out3 = vreinterpretq_f32_u64(vzip2q_u64(vreinterpretq_u64_f32(z01_h), vreinterpretq_u64_f32(z23_h)));

    // Store transposed 4x4 matrix
    vst1q_f32(out.as_mut_ptr(), out0);
    vst1q_f32(out.as_mut_ptr().add(out_stride), out1);
    vst1q_f32(out.as_mut_ptr().add(out_stride * 2), out2);
    vst1q_f32(out.as_mut_ptr().add(out_stride * 3), out3);
}

// ===========================================================================
// ORCHESTRATOR FUNCTIONS
// (These map exactly to the sse41 logic, just wrapping the NEON inner loops)
// ===========================================================================

#[cfg(target_arch = "aarch64")]
pub unsafe fn transpose_neon_u8(
    in_matrix: &[u8], out_matrix: &mut [u8], width: usize, height: usize
) {
    const BLOCK: usize = 8;

    if width < BLOCK || height < BLOCK {
        return crate::transpose::transpose_scalar(in_matrix, out_matrix, width, height);
    }

    let src_w = width;
    let src_h = height;
    let dest_w = height;

    let main_src_w = src_w - (src_w % BLOCK);
    let main_src_h = src_h - (src_h % BLOCK);
    let (main_dest, bottom_dest) = out_matrix.split_at_mut(main_src_w * dest_w);

    main_dest.par_chunks_mut(BLOCK * dest_w).enumerate().for_each(|(band_idx, dest_band)| {
        let dest_y_start = band_idx * BLOCK;

        for dest_x in (0..main_src_h).step_by(BLOCK) {
            unsafe {
                transpose_8_by_8_neon_u8(
                    &in_matrix[(dest_x * src_w) + dest_y_start..],
                    &mut dest_band[dest_x..],
                    src_w,
                    dest_w,
                );
            }
        }

        // Clean up the right fringe of this band
        for dest_x in main_src_h..src_h {
            for local_y in 0..BLOCK {
                dest_band[local_y * dest_w + dest_x] = in_matrix[dest_x * src_w + (dest_y_start + local_y)];
            }
        }
    });

    // Clean up the bottom fringe sequentially
    let start_dest_y = main_src_w;
    for local_y in 0..(src_w % BLOCK) {
        let dest_y = start_dest_y + local_y;
        for dest_x in 0..src_h {
            bottom_dest[local_y * dest_w + dest_x] = in_matrix[dest_x * src_w + dest_y];
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub unsafe fn transpose_neon_u16(
    in_matrix: &[u16], out_matrix: &mut [u16], width: usize, height: usize
) {
    const BLOCK: usize = 8;

    if width < BLOCK || height < BLOCK {
        return crate::transpose::transpose_scalar(in_matrix, out_matrix, width, height);
    }

    let src_w = width;
    let src_h = height;
    let dest_w = height;

    let main_src_w = src_w - (src_w % BLOCK);
    let main_src_h = src_h - (src_h % BLOCK);
    let (main_dest, bottom_dest) = out_matrix.split_at_mut(main_src_w * dest_w);

    main_dest.par_chunks_mut(BLOCK * dest_w).enumerate().for_each(|(band_idx, dest_band)| {
        let dest_y_start = band_idx * BLOCK;

        for dest_x in (0..main_src_h).step_by(BLOCK) {
            unsafe {
                transpose_8_by_8_neon_u16(
                    &in_matrix[(dest_x * src_w) + dest_y_start..],
                    &mut dest_band[dest_x..],
                    src_w,
                    dest_w,
                );
            }
        }

        for dest_x in main_src_h..src_h {
            for local_y in 0..BLOCK {
                dest_band[local_y * dest_w + dest_x] = in_matrix[dest_x * src_w + (dest_y_start + local_y)];
            }
        }
    });

    let start_dest_y = main_src_w;
    for local_y in 0..(src_w % BLOCK) {
        let dest_y = start_dest_y + local_y;
        for dest_x in 0..src_h {
            bottom_dest[local_y * dest_w + dest_x] = in_matrix[dest_x * src_w + dest_y];
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub unsafe fn transpose_neon_float(
    in_matrix: &[f32], out_matrix: &mut [f32], width: usize, height: usize
) {
    const BLOCK: usize = 4;

    if width < BLOCK || height < BLOCK {
        return crate::transpose::transpose_scalar(in_matrix, out_matrix, width, height);
    }

    let src_w = width;
    let src_h = height;
    let dest_w = height;

    let main_src_w = src_w - (src_w % BLOCK);
    let main_src_h = src_h - (src_h % BLOCK);
    let (main_dest, bottom_dest) = out_matrix.split_at_mut(main_src_w * dest_w);

    main_dest.par_chunks_mut(BLOCK * dest_w).enumerate().for_each(|(band_idx, dest_band)| {
        let dest_y_start = band_idx * BLOCK;

        for dest_x in (0..main_src_h).step_by(BLOCK) {
            unsafe {
                transpose_neon_float_4x4_inner(
                    &in_matrix[(dest_x * src_w) + dest_y_start..],
                    &mut dest_band[dest_x..],
                    src_w,
                    dest_w,
                );
            }
        }

        for dest_x in main_src_h..src_h {
            for local_y in 0..BLOCK {
                dest_band[local_y * dest_w + dest_x] = in_matrix[dest_x * src_w + (dest_y_start + local_y)];
            }
        }
    });

    let start_dest_y = main_src_w;
    for local_y in 0..(src_w % BLOCK) {
        let dest_y = start_dest_y + local_y;
        for dest_x in 0..src_h {
            bottom_dest[local_y * dest_w + dest_x] = in_matrix[dest_x * src_w + dest_y];
        }
    }
}
#[cfg(test)]
#[cfg(target_arch = "aarch64")]
mod neon_transpose_tests {
    use super::*;
    // Adjust this import path depending on where your scalar transpose actually lives
    use crate::transpose::transpose_scalar;

    /// A robust selection of sizes to test SIMD block boundaries and fringes
    fn test_dimensions() -> Vec<(usize, usize)> {
        vec![
            (1, 1),       // Tiny (bypasses SIMD)
            (3, 5),       // Smaller than an 8x8 or 4x4 block
            (4, 4),       // Exact match for f32 SIMD block
            (8, 8),       // Exact match for u8/u16 SIMD block
            (16, 16),     // Perfect multiple of blocks
            (9, 7),       // Awkward fringe
            (15, 17),     // Awkward fringe > block size
            (31, 33),     // Larger uneven boundaries
            (128, 128),   // Decent cache-sized chunk
        ]
    }

    #[test]
    fn test_transpose_neon_u8_equivalence() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }

        for (width, height) in test_dimensions() {
            let len = width * height;
            let input: Vec<u8> = (0..len).map(|i| (i % 255) as u8).collect();

            let mut scalar_out = vec![0u8; len];
            let mut simd_out = vec![0u8; len];

            transpose_scalar(&input, &mut scalar_out, width, height);
            unsafe {
                transpose_neon_u8(&input, &mut simd_out, width, height);
            }

            assert_eq!(
                scalar_out, simd_out,
                "NEON u8 Transpose mismatch at width={width} height={height}",
            );
        }
    }

    #[test]
    fn test_transpose_neon_u16_equivalence() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }

        for (width, height) in test_dimensions() {
            let len = width * height;
            let input: Vec<u16> = (0..len).map(|i| (i % 65535) as u16).collect();

            let mut scalar_out = vec![0u16; len];
            let mut simd_out = vec![0u16; len];

            transpose_scalar(&input, &mut scalar_out, width, height);
            unsafe {
                transpose_neon_u16(&input, &mut simd_out, width, height);
            }

            assert_eq!(
                scalar_out, simd_out,
                "NEON u16 Transpose mismatch at width={width} height={height}",
            );
        }
    }

    #[test]
    fn test_transpose_neon_f32_equivalence() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }

        for (width, height) in test_dimensions() {
            let len = width * height;
            let input: Vec<f32> = (0..len).map(|i| i as f32).collect();

            let mut scalar_out = vec![0.0f32; len];
            let mut simd_out = vec![0.0f32; len];

            transpose_scalar(&input, &mut scalar_out, width, height);
            unsafe {
                transpose_neon_float(&input, &mut simd_out, width, height);
            }

            assert_eq!(
                scalar_out, simd_out,
                "NEON f32 Transpose mismatch at width={width} height={height}",
            );
        }
    }
}