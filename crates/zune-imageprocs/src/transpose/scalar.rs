/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

pub fn transpose_scalar<T: Copy + Default>(
    in_matrix: &[T], out_matrix: &mut [T], width: usize, height: usize
) {
    // A slightly more optimized scalar transpose,
    // 2x faster than the naive one
    //
    // The only difference with the naive is that you
    // do tiling transpose, this allows us to use the cache better
    // at the compromise that is is complicated.
    //
    // The gist of it is that we do scalar a single 8 by 8 transpose and write to an immediate
    // buffer and then write that buffer to our destination
    let dimensions = width * height;
    assert_eq!(
        in_matrix.len(),
        dimensions,
        "In matrix dimensions do not match width and height"
    );

    assert_eq!(
        out_matrix.len(),
        dimensions,
        "Out matrix dimensions do not match width and height"
    );

    let mut temp_matrix: [T; 64] = [T::default(); 64];

    let width_iterations = width / 8;
    let sin_height = 8 * width;

    for (i, in_width_stride) in in_matrix.chunks_exact(sin_height).enumerate() {
        for j in 0..width_iterations {
            let out_height_stride = &mut out_matrix[(j * height * 8) + (i * 8)..];

            let in_width = in_width_stride[(j * 8)..].chunks(width);

            for (k, w_d) in in_width.enumerate().take(8) {
                for (l, h) in w_d.iter().enumerate().take(8) {
                    temp_matrix[(l * 8) + k] = *h;
                    // Optimizer is really trying stuff here
                    // Not a perf boost but a code bloat, so tell it
                    // to listen to me, the MASTER.
                    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                    unsafe {
                        std::arch::asm!("");
                    }
                }
            }
            // copy out height stride in chunks of 8
            let mut stride_start = 0;

            for small_m in temp_matrix.chunks_exact(8) {
                out_height_stride[stride_start..stride_start + 8].copy_from_slice(small_m);
                stride_start += height;
            }
        }
    }
    let rem_w = width - (width & 7) - 1;
    let rem_h = height - (height & 7) - 1;

    for i in rem_h..height {
        for j in 0..width {
            out_matrix[(j * height) + i] = in_matrix[(i * width) + j];
        }
    }
    for i in rem_w..width {
        for j in 0..height {
            out_matrix[(i * height) + j] = in_matrix[(j * width) + i];
        }
    }
}
