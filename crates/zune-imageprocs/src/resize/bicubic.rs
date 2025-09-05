#![allow(dead_code)]

use crate::{
    mathops::{powi_f32, trunc_f32},
    traits::NumOps,
};

#[cfg(feature = "portable-simd")]
mod std_simd {
    use std::simd::cmp::SimdPartialOrd;
    use std::simd::f32x4;
    use std::simd::num::SimdFloat;

    #[inline(always)]
    pub fn bicubic_kernel_simd_inner(x: f32x4) -> [f32; 4] {
        let a = f32x4::splat(-0.5);
        let x = x.abs();

        let powi2 = x * x;
        let powi3 = powi2 * x;

        // (A + 2.0) * x.powi(3) - (A + 3.0) * x.powi(2) + 1.0
        let p1 = ((a + f32x4::splat(2.0)) * powi3) - ((a + f32x4::splat(3.0)) * powi2)
            + f32x4::splat(1.0);
        // A * x.powi(3) - 5.0 * A * x.powi(2) + 8.0 * A * x - 4.0 * A
        let p2 = a * powi3 - f32x4::splat(5.0) * a * powi2 + f32x4::splat(8.0) * a * x
            - f32x4::splat(4.0) * a;
        // 0
        let p3 = f32x4::splat(0.0);
        // x  < 1
        let m1 = x.simd_le(f32x4::splat(1.0));
        // x <= 2;
        let m2 = x.simd_lt(f32x4::splat(2.0));
        // x  > 3;
        let m3 = !m2;

        let final_value = m3.select(p3, m1.select(p1, p2));

        p1.to_array().into_iter().reduce(|x, a| x + a);

        return final_value.to_array();
    }

    #[inline]
    pub fn bicubic_kernel_simd(y0: isize, src_y: f32) -> [f32; 4] {
        // Similar to the normal bicubic kernel calculation
        // but we flatten the calculations and remove the if's
        // choosing blends/masks which increase ilp.

        // benchmarks
        //
        // test resize::bicubic::benchmarks::bench_resize_cubic_normal    ... bench:   8,668,445.80 ns/iter (+/- 78,967.59)
        // test resize::bicubic::benchmarks::bench_resize_cubic_simd      ... bench:   2,102,972.90 ns/iter (+/- 13,304.78)
        //
        // Machine: Apple Macbook pro m3
        // Reproducible by running `cargo bench  "resize" --features benchmarks,portable-simd`

        let yc = f32x4::splat(y0 as f32);
        let yy = yc + f32x4::from_array([-1., 0., 1., 2.]);
        let src_y = f32x4::splat(src_y);
        let x = src_y - yy;

        bicubic_kernel_simd_inner(x)
    }
}
const A: f32 = -0.5;

#[inline]
fn bicubic_kernel(x: f32) -> f32 {
    let x = x.abs();
    if x <= 1.0 {
        (A + 2.0) * powi_f32(x, 3) - (A + 3.0) * powi_f32(x, 2) + 1.0
    } else if x < 2.0 {
        A * powi_f32(x, 3) - 5.0 * A * powi_f32(x, 2) + 8.0 * A * x - 4.0 * A
    } else {
        0.0
    }
}

#[inline]
pub fn bicubic_scalar(x: [f32; 4]) -> [f32; 4] {
    let mut out = [0.0; 4];
    for i in 0..4 {
        out[i] = bicubic_kernel(x[i]);
    }
    return out;
}

#[inline]
fn inline_floor(x: f32) -> f32 {
    let i32_x = x as i32;

    return (i32_x - i32::from(x < i32_x as f32)) as f32; // as dgobbi above, needs less than for floor
}

#[allow(unreachable_code)]
#[inline]
fn bicubic_function(y0: isize, src_y: f32) -> [f32; 4] {
    #[cfg(feature = "portable-simd")]
    {
        use std_simd::bicubic_kernel_simd;
        return bicubic_kernel_simd(y0, src_y);
    }
    let yy0 = y0 + -1;
    let yy1 = y0 + 0;
    let yy2 = y0 + 1;
    let yy3 = y0 + 2;

    let dy0 = src_y - yy0 as f32;
    let dy1 = src_y - yy1 as f32;
    let dy2 = src_y - yy2 as f32;
    let dy3 = src_y - yy3 as f32;
    return bicubic_scalar([dy0, dy1, dy2, dy3]);
}
pub fn bicubic_resample<T>(
    input: &[T], output: &mut [T], input_width: usize, input_height: usize, new_width: usize,
    new_height: usize,
) where
    T: Copy + NumOps<T>,
    f32: core::convert::From<T>,
{
    let scale_y = input_height as f32 / new_height as f32;
    let scale_x = input_width as f32 / new_width as f32;

    // pre-calculate all the x coefficients, since they will be repeated for every y-value
    //
    // Before
    //  test resize::benchmarks::bench_resize_cubic     ... bench:  18,673,279.20 ns/iter (+/- 1,320,342.58)
    //
    // After
    //  test resize::benchmarks::bench_resize_cubic     ... bench:  12,923,833.40 ns/iter (+/- 528,285.41)
    let mut x_mega_coeffs = vec![[0.0; 4]; new_width];

    for x in 0..new_width {
        let src_x = x as f32 * scale_x;
        let x0 = inline_floor(src_x) as isize;

        x_mega_coeffs[x] = bicubic_function(x0, src_x);
    }

    for (y, output_stride) in (0..new_height).zip(output.chunks_exact_mut(new_width)) {
        let src_y = y as f32 * scale_y;
        // the ideal one is src_y.floor(), but
        // trunk == floor for +ve values and
        // src_y can't be negative.
        let y0 = trunc_f32(src_y) as isize;

        let y_coeffs = bicubic_function(y0, src_y);

        for (x, x_coeffs) in (0..new_width).zip(x_mega_coeffs.iter()) {
            let src_x = x as f32 * scale_x;
            let x0 = trunc_f32(src_x) as usize;
            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            if (y0 > 0 && y0 + 2 < input_height as isize) && (x0 > 0 && x0 + 2 < input_width) {
                // common happy path
                // inside the boundaries
                let mut yy = y0 - 1;
                // Improves perf by 15% in m3 so important
                for weight_y in y_coeffs {
                    let width_stride = yy as usize * input_width;
                    let start = x0 + width_stride;
                    let input_stride_c = &input[start - 1..start + 3];

                    debug_assert!(input_stride_c.len() == 4);

                    for (pix, weight_x) in input_stride_c.iter().zip(x_coeffs) {
                        let weight = weight_y * weight_x;
                        let pixel = f32::from(*pix);
                        sum += pixel * weight;
                        weight_sum += weight;
                    }
                    yy += 1;
                }
                // [cae]: I assume weight sum can never be zero, since we must multiply something
                debug_assert!(weight_sum != 0.0);
                if weight_sum != 0.0 {
                    output_stride[x] = T::from_f32(sum / weight_sum);
                }
            } else {
                // path that is taken in boundaries
                // less taken but has to be careful to always be in bounds
                for ky in -1..=2 {
                    let yy = y0 + ky;

                    if yy >= 0 && yy < input_height as isize {
                        let weight_y = y_coeffs[(ky + 1) as usize];
                        let width_stride = yy as usize * input_width;

                        for kx in -1..=2 {
                            let xx = (x0 as isize) + kx;

                            if xx >= 0 && xx < input_width as isize {
                                let weight_x = x_coeffs[(kx + 1) as usize];
                                let weight = weight_y * weight_x;

                                let pixel = f32::from(input[width_stride + xx as usize]);

                                sum += pixel * weight;
                                weight_sum += weight;
                            }
                        }
                    }
                }
                output_stride[x] =
                    if weight_sum > 0.0 { T::from_f32(sum / weight_sum) } else { T::from_f32(0.0) };
            }
        }
    }
}

#[cfg(all(feature = "benchmarks", feature = "portable-simd"))]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use core::hint::black_box;
    use std::simd::f32x4;

    use nanorand::{Rng, WyRand};

    use crate::resize::bicubic::bicubic_scalar;
    use crate::resize::bicubic::std_simd::bicubic_kernel_simd_inner;

    #[bench]
    fn bench_resize_cubic_simd(b: &mut test::Bencher) {
        let width = 4000;
        let height = 2000;

        let dimensions = width * height;

        let mut in_vec = vec![0_f32; dimensions];

        // fill with random bytes
        let mut rand = WyRand::new();
        in_vec
            .iter_mut()
            .for_each(|entry| *entry = rand.generate_range(0..25) as f32 / 10.0);

        let simd_exec = |x: &[f32]| {
            let mut out = [0.; 4];
            for chunk in x.chunks_exact(4) {
                out.copy_from_slice(chunk);
                let c = f32x4::from_array(out);
                black_box(bicubic_kernel_simd_inner(c));
            }
        };

        b.iter(|| black_box(simd_exec(&in_vec)));
    }

    #[bench]
    fn bench_resize_cubic_normal(b: &mut test::Bencher) {
        let width = 4000;
        let height = 2000;

        let dimensions = width * height;

        let mut in_vec = vec![0_f32; dimensions];

        // fill with random bytes
        let mut rand = WyRand::new();
        in_vec
            .iter_mut()
            .for_each(|entry| *entry = rand.generate_range(0..25) as f32 / 10.0);

        let simd_exec = |x: &[f32]| {
            let mut out = [0.; 4];
            for chunk in x.chunks_exact(4) {
                out.copy_from_slice(chunk);
                black_box(bicubic_scalar(out));
            }
        };

        b.iter(|| black_box(simd_exec(&in_vec)));
    }
}
