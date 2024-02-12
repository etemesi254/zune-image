#![allow(dead_code)]

use crate::traits::NumOps;
pub fn resize_image_bicubic<T>(
    _pixels: &[T], _output: &mut [T], _width: usize, _height: usize, _new_width: usize,
    _new_height: usize
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
{
    // // Filter coefficients for bicubic interpolation with Mitchell-Netravali kernel
    // #[rustfmt::skip]
    // let filter_coefficients = [
    //     -0.772, 0.270, -0.024, 0.006,
    //     0.826, -0.688, 0.491, -0.090,
    //     -0.254, 0.870, 0.647, -0.166,
    //     0.064, -0.703, 0.728,  0.319
    // ];
    //
    // for y in 0..new_height {
    //     for x in 0..new_width {
    //         let new_x = x as f32 / new_width as f32 * width as f32;
    //         let new_y = y as f32 / new_height as f32 * height as f32;
    //
    //         let x0 = (new_x - 1.0).floor() as usize;
    //         let x1 = x0 + 1;
    //         let x2 = x1 + 1;
    //         let x3 = x2 + 1;
    //
    //         let y0 = (new_y - 1.0).floor() as usize;
    //         let y1 = y0 + 1;
    //         let y2 = y1 + 1;
    //         let y3 = y2 + 1;
    //
    //         // Clamp pixel indices to image boundaries
    //         let x0 = x0.min(width - 1);
    //         let x3 = x3.min(width - 1);
    //         let y3 = y3.min(height - 1);
    //
    //         // Calculate cubic coefficients
    //         let mut a_coeffs = [0.0; 4];
    //         for i in 0..4 {
    //             a_coeffs[i] =
    //                 calculate_cubic_coefficient(new_x - x0 as f32, filter_coefficients[i * 4]);
    //         }
    //
    //         // Interpolate pixel values
    //         let mut a = 0.0;
    //         for i in 0..4 {
    //             for j in 0..4 {
    //                 let offset = (y3 - i) * width + (x3 - j);
    //                 a += f32::from(pixels[offset]) * a_coeffs[i] * a_coeffs[j];
    //             }
    //         }
    //
    //         output[y * new_width + x] = T::from_f32(a);
    //     }
    // }
}

fn calculate_cubic_coefficient(x: f32, a: f32) -> f32 {
    return if x < 1.0 {
        a * x * x * x + (a + 2.0) * x * x + (a + 1.0) * x
    } else if x < 2.0 {
        -a * x * x * x + (5.0 * a + 2.0) * x * x - (8.0 * a + 4.0) * x + (4.0 * a + 6.0)
    } else {
        0.0
    };
}
