// /*
//  * Copyright (c) 2023.
//  *
//  * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
//  */
//
// //! RGB to XYB conversions
// //!
// //! This module includes functions to convert RGB<->XYB
// //!
// //! XYB is a colorspace used by jpeg-xl which is derived from
// //! [LMS](https://en.wikipedia.org/wiki/LMS_color_space)
// //! and this module allows us to work with such colorspaces
// //!
// #![allow(
// clippy::unreadable_literal,
// clippy::excessive_precision,
// clippy::many_single_char_names
// )]
//
// use crate::traits::{NumOps, ZFloat};
//
// // Unscaled values for K_OPSIN_ABSORBANCE_BIAS
// const K_B0: f64 = 0.96723368009523958;
// const K_B1: f64 = K_B0;
// const K_B2: f64 = K_B0;
// const K_SCALE: f64 = 255.0;
// const K_INV_SCALE_R: f64 = 1.0;
// const K_INV_SCALE_G: f64 = 1.0;
//
// const K_OPSIN_ABSORBANCE_BIAS: [f64; 3] = [K_B0 / K_SCALE, K_B1 / K_SCALE, K_B2 / K_SCALE];
//
// #[rustfmt::skip]
// // Must be the inverse matrix of K_OPSIN_ABSORBANCE_MATRIX and match the spec.
// static K_DEFAULT_INVERSE_OPSIN_ABSORBANCE_MATRIX: [f64; 9] = [
//     2813.04956, -2516.070700, -041.9788641,
//     -829.807582, 1126.786450, -041.9788641,
//     -933.007078, 0691.795377, 496.2117010
// ];
//
// #[rustfmt::skip]
// static K_NEG_OPSIN_ABSORBANCE_BIAS_RGB: [f64; 4] = [
//     -K_OPSIN_ABSORBANCE_BIAS[0], -K_OPSIN_ABSORBANCE_BIAS[1],
//     -K_OPSIN_ABSORBANCE_BIAS[2], 255.0
// ];
//
// #[rustfmt::skip]
// static K_NEG_OPSIN_ABSORBANCE_BIAS_CBRT: [f64; 4] = [
//     -0.1559542005492486, -0.1559542005492486,
//     -0.1559542005492486, 6.3413257053849980
// ];
//
// #[rustfmt::skip]
// static PREMUL_ABSORB: [f64; 12] = [
//     0.0011764705882352940, 0.0024392156862745100, 0.00030588235294117644,
//     0.0009019607843137256, 0.0027137254901960788, 0.00030588235294117644,
//     0.0009545987813548164, 0.0008030095852743851, 0.00216396026082177900,
//     -0.1559542005492486000, -0.1559542005492486000, -0.15595420054924860000
// ];
//
// #[inline]
// fn mul_add(a: f64, b: f64, c: f64) -> f64
// {
//     return a * b + c;
// }
//
// #[inline]
// fn opsin_absorbance(r: f64, g: f64, b: f64) -> [f64; 3]
// {
//     let bias = &K_OPSIN_ABSORBANCE_BIAS;
//     let m = &PREMUL_ABSORB;
//     let mixed0 = mul_add(m[0], r, mul_add(m[1], g, mul_add(m[2], b, bias[0])));
//     let mixed1 = mul_add(m[3], r, mul_add(m[4], g, mul_add(m[5], b, bias[1])));
//     let mixed2 = mul_add(m[6], r, mul_add(m[7], g, mul_add(m[8], b, bias[2])));
//     return [mixed0, mixed1, mixed2];
// }
//
// fn gamma_to_linear(n: f64) -> f64
// {
//     return if n <= 0.0404482362771082
//     {
//         n / 12.92
//     }
//     else
//     {
//         ((n + 0.055) / 1.055).powf(2.4)
//     };
// }
//
// fn linear_to_gamma(n: f64) -> f64
// {
//     return if n <= 0.00313066844250063
//     {
//         n * 12.92
//     }
//     else
//     {
//         1.055 * n.powf(1.0 / 2.4) - 0.055
//     };
// }
//
// /// Convert a single RGB triple to XYB
// #[inline]
// fn rgb_to_xyb_single(r: f64, g: f64, b: f64) -> [f64; 3]
// {
//     let (l_r, l_g, l_b) = (
//         255.0 * gamma_to_linear(r / 255.0),
//         255.0 * gamma_to_linear(g / 255.0),
//         255.0 * gamma_to_linear(b / 255.0)
//     );
//     let [mut mixed0, mut mixed1, mut mixed2] = opsin_absorbance(l_r, l_g, l_b);
//
//     // mixed should be non-negative even for wide-gamut, so clamp to zero.
//     mixed0 = mixed0.max(0.0);
//     mixed1 = mixed1.max(0.0);
//     mixed2 = mixed2.max(0.0);
//
//     // CubeRootAndAdd
//     mixed0 = mixed0.cbrt() + PREMUL_ABSORB[9];
//     mixed1 = mixed1.cbrt() + PREMUL_ABSORB[10];
//     mixed2 = mixed2.cbrt() + PREMUL_ABSORB[11];
//
//     return linear_xyb_transform(mixed0, mixed1, mixed2);
// }
//
// #[inline]
// fn linear_xyb_transform(r: f64, g: f64, b: f64) -> [f64; 3]
// {
//     return [0.5 * (r - g), 0.5 * (r + g), b];
// }
//
// /// Convert a single XYB triple to RGB
// #[inline]
// fn xyb_to_rgb_single(x: f64, y: f64, b: f64) -> [f64; 3]
// {
//     let [opsin_x, opsin_y, opsin_b] = [x, y, b];
//     let inv_scale_x = K_INV_SCALE_R;
//     let inv_scale_y = K_INV_SCALE_G;
//     let neg_bias_r = K_NEG_OPSIN_ABSORBANCE_BIAS_RGB[0];
//     let neg_bias_g = K_NEG_OPSIN_ABSORBANCE_BIAS_RGB[1];
//     let neg_bias_b = K_NEG_OPSIN_ABSORBANCE_BIAS_RGB[2];
//
//     // Color space: XYB -> RGB
//     let mut gamma_r = inv_scale_x * (opsin_y + opsin_x);
//     let mut gamma_g = inv_scale_y * (opsin_y - opsin_x);
//     let mut gamma_b = opsin_b;
//
//     gamma_r -= K_NEG_OPSIN_ABSORBANCE_BIAS_CBRT[0];
//     gamma_g -= K_NEG_OPSIN_ABSORBANCE_BIAS_CBRT[1];
//     gamma_b -= K_NEG_OPSIN_ABSORBANCE_BIAS_CBRT[2];
//
//     // Undo gamma compression: linear = gamma^3 for efficiency.
//     let gamma_r2 = gamma_r * gamma_r;
//     let gamma_g2 = gamma_g * gamma_g;
//     let gamma_b2 = gamma_b * gamma_b;
//     let mixed_r = mul_add(gamma_r2, gamma_r, neg_bias_r);
//     let mixed_g = mul_add(gamma_g2, gamma_g, neg_bias_g);
//     let mixed_b = mul_add(gamma_b2, gamma_b, neg_bias_b);
//
//     let inverse_matrix = &K_DEFAULT_INVERSE_OPSIN_ABSORBANCE_MATRIX;
//
//     // Unmix (multiply by 3x3 inverse_matrix)
//     let (mut linear_r, mut linear_g, mut linear_b);
//     linear_r = inverse_matrix[0] * mixed_r;
//     linear_g = inverse_matrix[3] * mixed_r;
//     linear_b = inverse_matrix[6] * mixed_r;
//     linear_r = mul_add(inverse_matrix[1], mixed_g, linear_r);
//     linear_g = mul_add(inverse_matrix[4], mixed_g, linear_g);
//     linear_b = mul_add(inverse_matrix[7], mixed_g, linear_b);
//     linear_r = mul_add(inverse_matrix[2], mixed_b, linear_r);
//     linear_g = mul_add(inverse_matrix[5], mixed_b, linear_g);
//     linear_b = mul_add(inverse_matrix[8], mixed_b, linear_b);
//
//     return [
//         (linear_to_gamma(linear_r / 255.0) * 255.0).round(),
//         (linear_to_gamma(linear_g / 255.0) * 255.0).round(),
//         (linear_to_gamma(linear_b / 255.0) * 255.0).round()
//     ];
// }
//
// /// Convert from the RGB colorspace to XYB
// ///
// /// Output array is expected to be a Float type, i.e f32 or f64,
// /// since XYB works in the float domain
// pub fn rgb_to_xyb_channel<T, U>(r: &[T], g: &[T], b: &[T], x: &mut [U], y: &mut [U], xb: &mut [U])
//     where
//         T: Copy + NumOps<T>,
//         U: Copy + NumOps<U> + ZFloat
// {
//     for (((((r, g), b), x), y), z) in r
//         .iter()
//         .zip(g)
//         .zip(b)
//         .zip(x.iter_mut())
//         .zip(y.iter_mut())
//         .zip(xb.iter_mut())
//     {
//         let (r_f, g_f, b_f) = (r.to_f64(), g.to_f64(), b.to_f64());
//         //convert
//         let result = rgb_to_xyb_single(r_f, g_f, b_f).map(|x| U::from_f64(x));
//         // write output
//         *x = result[0];
//         *y = result[1];
//         *z = result[2];
//     }
// }
//
// /// Convert from the RGB colorspace to XYB
// ///
// /// Input arrays (X,Y,B) are expected to be a Float type, i.e f32 or f64,
// /// since XYB works in the float domain
// pub fn xyb_to_rgb_channel<T, U>(x: &[T], y: &[T], xb: &[T], r: &mut [U], g: &mut [U], b: &mut [U])
//     where
//         T: Copy + NumOps<T> + ZFloat,
//         U: Copy + NumOps<U>
// {
//     for (((((x, y), xb), r), g), b) in x
//         .iter()
//         .zip(y)
//         .zip(xb)
//         .zip(r.iter_mut())
//         .zip(g.iter_mut())
//         .zip(b.iter_mut())
//     {
//         let (x_f, y_f, b_f) = (x.to_f64(), y.to_f64(), xb.to_f64());
//         //convert
//         let result = xyb_to_rgb_single(x_f, y_f, b_f).map(|x| U::from_f64(x));
//         // write to output
//         *r = result[0];
//         *g = result[1];
//         *b = result[2];
//     }
// }
//
// /// Convert from RGB to XYB colorspace
// ///
// /// Output array is expected to be a Float type, i.e f32 or f64,
// /// since XYB works in the float domain
// pub fn rgb_to_xyb<T, U>(rgb: &[T], xyb: &mut [U])
//     where
//         T: Copy + NumOps<T>,
//         U: Copy + NumOps<U> + ZFloat
// {
//     for (rgb_c, xyb_c) in rgb.chunks_exact(3).zip(xyb.chunks_exact_mut(3))
//     {
//         let (r, g, b) = (rgb_c[0].to_f64(), rgb_c[1].to_f64(), rgb_c[2].to_f64());
//
//         // convert and map back to U
//         let result = rgb_to_xyb_single(r, g, b).map(|x| U::from_f64(x));
//         // copy to slice
//         xyb_c.copy_from_slice(&result);
//     }
// }
//
// /// Convert from XYB to RGB
// ///
// /// Input array xyb is expected to be a Float type, i.e f32 or f64,
// /// since XYB works in the float domain
// pub fn xyb_to_rgb<T, U>(xyb: &[T], rgb: &mut [U])
//     where
//         T: Copy + NumOps<T> + ZFloat,
//         U: Copy + NumOps<U>
// {
//     for (xyb_c, rgb_c) in xyb.chunks_exact(3).zip(rgb.chunks_exact_mut(3))
//     {
//         let (x, y, b) = (xyb_c[0].to_f64(), xyb_c[1].to_f64(), xyb_c[2].to_f64());
//
//         // convert and map back to U
//         let result = xyb_to_rgb_single(x, y, b).map(|x| U::from_f64(x));
//         // copy to slice
//         rgb_c.copy_from_slice(&result);
//     }
// }
//
// #[test]
// fn test_rgb_to_xyb()
// {
//     #[rustfmt::skip]
//         let input = [
//         [30.0, 20.0, 10.0],
//         [37.0, 37.0, 37.0],
//         [27.0, 27.0, 15.0],
//         [21.0, 13.0, 15.0]
//     ];
//     #[rustfmt::skip]
//         let output = [
//         [0.00132793167336260620, 0.073401900355357000, 0.059924746902774745],
//         [0.00000000000000000000, 0.125489420959740940, 0.125489420959740900],
//         [-1.3877787807814457e-17, 0.086604546974747000, 0.068720221161379960],
//         [0.00095119542028948570, 0.050475213533708235, 0.052658031023773170]
//     ];
//
//     for (inp, out) in input.iter().zip(output)
//     {
//         let [r, g, b] = *inp;
//
//         let result = rgb_to_xyb_single(r, g, b);
//
//         result.iter().enumerate().for_each(|(pos, x)| {
//             assert!(
//                 (f64::EPSILON > (out[pos] - x)),
//                 "Conflicting answers, expected {}, found {}",
//                 out[pos],
//                 x
//             );
//         });
//     }
// }
//
// #[test]
// fn test_xyb_to_rgb()
// {
//     // the same test as above, with output and input reversed
//     // confirms we round-trip
//     #[rustfmt::skip]
//         let output = [
//         [30.0, 20.0, 10.0],
//         [37.0, 37.0, 37.0],
//         [27.0, 27.0, 15.0],
//         [21.0, 13.0, 15.0]
//     ];
//     #[rustfmt::skip]
//         let input = [
//         [0.00132793167336260620, 0.073401900355357000, 0.059924746902774745],
//         [0.00000000000000000000, 0.125489420959740940, 0.125489420959740900],
//         [-1.3877787807814457e-17, 0.086604546974747000, 0.068720221161379960],
//         [0.00095119542028948570, 0.050475213533708235, 0.052658031023773170]
//     ];
//
//     for (inp, out) in input.iter().zip(output)
//     {
//         let [r, g, b] = *inp;
//
//         let result = xyb_to_rgb_single(r, g, b);
//
//         result.iter().enumerate().for_each(|(pos, x)| {
//             assert!(
//                 (f64::EPSILON > (out[pos] - x)),
//                 "Conflicting answers, expected {}, found {}",
//                 out[pos],
//                 x
//             );
//         });
//     }
// }
