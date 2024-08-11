#![allow(dead_code)]

use crate::traits::NumOps;


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
        let mut x_coeffs = [0.0; 4];
        let mut i = 0;
        for kx in -1..=2 {
            let xx = x0 + kx;
            let dx = src_x - xx as f32;

            let weight_x = bicubic_kernel(dx);
            x_coeffs[i] = weight_x;

            i += 1;
        }
        x_mega_coeffs[x] = x_coeffs;
    }

    for y in 0..new_height {
        let src_y = y as f32 * scale_y;
        let y0 = src_y.floor() as isize;

        // pre-calculate the y-coeff kernels
        let mut y_coeffs = [0.0; 4];
        let mut i = 0;

        for ky in -1..=2 {
            let yy = y0 + ky;
            let dy = src_y - yy as f32;
            let weight_y = bicubic_kernel(dy);
            y_coeffs[i] = weight_y;
            i += 1;
        }

        for x in 0..new_width {
            let src_x = x as f32 * scale_x;
            let x0 = src_x.floor() as isize;

            let x_coeffs = x_mega_coeffs[x];

            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            for ky in -1..=2 {
                let yy = y0 + ky;

                if yy >= 0 && yy < input_height as isize {
                    let weight_y = y_coeffs[(ky + 1) as usize];

                    for kx in -1..=2 {
                        let xx = x0 + kx;

                        if xx >= 0 && xx < input_width as isize {
                            let weight_x = x_coeffs[(kx + 1) as usize];
                            let weight = weight_y * weight_x;

                            let pixel = f32::from(input[yy as usize * input_width + xx as usize]);

                            sum += pixel * weight;
                            weight_sum += weight;
                        }
                    }
                }
            }

            output[y * new_width + x] = if weight_sum > 0.0 { T::from_f32(sum / weight_sum) } else { T::from_f32(0.0) };
        }
    }
}

