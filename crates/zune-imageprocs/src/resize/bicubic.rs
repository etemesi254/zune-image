#![allow(dead_code)]


use crate::traits::NumOps;

#[cfg(feature = "portable-simd")]
mod std_simd {
    use std::simd::num::SimdFloat;
    use std::simd::{f32x4};
    use std::simd::cmp::SimdPartialOrd;

    #[inline]
    pub fn bicubic_kernel_simd(x: [f32; 4]) -> [f32; 4] {
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

        let x = f32x4::from_array(x);

        let a = f32x4::splat(-0.5);
        let x = x.abs();

        let powi2 = x * x;
        let powi3 = powi2 * x;


        // (A + 2.0) * x.powi(3) - (A + 3.0) * x.powi(2) + 1.0
        let p1 = ((a + f32x4::splat(2.0)) * powi3) - ((a + f32x4::splat(3.0)) * powi2) + f32x4::splat(1.0);
        // A * x.powi(3) - 5.0 * A * x.powi(2) + 8.0 * A * x - 4.0 * A
        let p2 = a * powi3 - f32x4::splat(5.0) * a * powi2 + f32x4::splat(8.0) * a * x - f32x4::splat(4.0) * a;
        // 0
        let p3 = f32x4::splat(0.0);
        // x  < 1
        let m1 = x.simd_le(f32x4::splat(1.0));
        // x <= 2;
        let m2 = x.simd_lt(f32x4::splat(2.0));
        // x  > 3;
        let m3 = !m2;

        let final_value = m3.select(p3, m1.select(p1, p2));

        return final_value.to_array();
    }
}
const A: f32 = -0.5;

#[inline]
fn bicubic_kernel(x: f32) -> f32 {
    let x = x.abs();
    if x <= 1.0 {
        (A + 2.0) * x.powi(3) - (A + 3.0) * x.powi(2) + 1.0
    } else if x < 2.0 {
        A * x.powi(3) - 5.0 * A * x.powi(2) + 8.0 * A * x - 4.0 * A
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

#[allow(unreachable_code)]
#[inline]
fn bicubic_function(x: [f32; 4]) -> [f32; 4] {
    #[cfg(feature = "portable-simd")]
    {
        use std_simd::bicubic_kernel_simd;
        return bicubic_kernel_simd(x);
    }
    return bicubic_scalar(x);
}
pub fn bicubic_resample<T>(input: &[T], output: &mut [T], input_width: usize, input_height: usize, new_width: usize, new_height: usize)
where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
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
        let x0 = src_x.floor() as isize;

        // pre-calculate the x-coeff kernels
        let xx0 = x0 + -1;
        let xx1 = x0 + 0;
        let xx2 = x0 + 1;
        let xx3 = x0 + 2;

        let dx0 = src_x - xx0 as f32;
        let dx1 = src_x - xx1 as f32;
        let dx2 = src_x - xx2 as f32;
        let dx3 = src_x - xx3 as f32;

        let array = [dx0, dx1, dx2, dx3];

        x_mega_coeffs[x] = bicubic_function(array);
    }

    for (y, output_stride) in (0..new_height).zip(output.chunks_exact_mut(new_width)) {
        let src_y = y as f32 * scale_y;
        let y0 = src_y.floor() as isize;

        // pre-calculate the y-coeff kernels
        let yy0 = y0 + -1;
        let yy1 = y0 + 0;
        let yy2 = y0 + 1;
        let yy3 = y0 + 2;

        let dy0 = src_y - yy0 as f32;
        let dy1 = src_y - yy1 as f32;
        let dy2 = src_y - yy2 as f32;
        let dy3 = src_y - yy3 as f32;


        let y_coeffs = bicubic_function([dy0, dy1, dy2, dy3]);

        for (x, x_coeffs) in (0..new_width).zip(x_mega_coeffs.iter()) {
            let src_x = x as f32 * scale_x;
            let x0 = src_x.floor() as isize;

            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            if (y0 > 0 && y0 + 2 < input_height as isize) && (x0 > 0 && x0 + 2 < input_width as isize) {
                // common happy path
                // inside the boundaries
                //
                // Improves perf by 15% in m3 so important
                for ky in -1..=2 {
                    let yy = y0 + ky;
                    let width_stride = yy as usize * input_width;
                    let weight_y = y_coeffs[(ky + 1) as usize];

                    for kx in -1..=2 {
                        let xx = x0 + kx;

                        let weight_x = x_coeffs[(kx + 1) as usize];
                        let weight = weight_y * weight_x;

                        let pixel = f32::from(input[width_stride + xx as usize]);
                        sum += pixel * weight;
                        weight_sum += weight;
                    }
                }
                // [cae]: I assume weight sum can never be zero, since we must multiply something
                debug_assert!(weight_sum != 0.0);
                output_stride[x] = T::from_f32(sum / weight_sum);
            } else {
                // path that is taken in boundaries
                // less taken but has to be careful to always be in bounds
                for ky in -1..=2 {
                    let yy = y0 + ky;

                    if yy >= 0 && yy < input_height as isize {
                        let weight_y = y_coeffs[(ky + 1) as usize];
                        let width_stride = yy as usize * input_width;

                        for kx in -1..=2 {
                            let xx = x0 + kx;

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
                output_stride[x] = if weight_sum > 0.0 { T::from_f32(sum / weight_sum) } else { T::from_f32(0.0) };
            }
        }
    }
}


#[cfg(all(feature = "benchmarks", feature = "portable-simd"))]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use std::hint::black_box;
    use nanorand::{Rng, WyRand};
    use crate::resize::bicubic::bicubic_scalar;
    use crate::resize::bicubic::std_simd::bicubic_kernel_simd;

    #[bench]
    fn bench_resize_cubic_simd(b: &mut test::Bencher) {
        let width = 4000;
        let height = 2000;

        let dimensions = width * height;

        let mut in_vec = vec![0_f32; dimensions];

        // fill with random bytes
        WyRand::new().fill(&mut in_vec);

        let simd_exec = |x: &[f32]| {
            let mut out = [0.; 4];
            for chunk in x.chunks_exact(4) {
                out.copy_from_slice(chunk);
                black_box(bicubic_kernel_simd(out));
            }
        };

        b.iter(|| {
            black_box(simd_exec(&in_vec))
        });
    }

    #[bench]
    fn bench_resize_cubic_normal(b: &mut test::Bencher) {
        let width = 4000;
        let height = 2000;

        let dimensions = width * height;

        let mut in_vec = vec![0_f32; dimensions];

        // fill with random bytes
        WyRand::new().fill(&mut in_vec);

        let simd_exec = |x: &[f32]| {
            let mut out = [0.; 4];
            for chunk in x.chunks_exact(4) {
                out.copy_from_slice(chunk);
                black_box(bicubic_scalar(out));
            }
        };

        b.iter(|| {
            black_box(simd_exec(&in_vec))
        });
    }
}
