/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use crate::pad::{pad, PadMethod};
use crate::spatial::{spatial, spatial_NxN};
use crate::traits::NumOps;

/// Calculate scharr for f32 images
///
/// # Arguments
/// -  in_values: An array which is expected to contain 9 elements 
///  that represents a 3x3 window for which we are to calculate the sobel 
#[rustfmt::skip]
fn scharr_inner_f32<T>(c: &[T; 9]) -> T
    where
        T: NumOps<T> + Copy + Default,
        f32: std::convert::From<T>
{
    // matrix
    //   -3, 0,  3,
    //  -10, 0, 10,
    //   -3, 0,  3
    //
    let mut sum_a = 0.0;
    sum_a += (f32::from(c[0]) * -03.) + (f32::from(c[2]) * 03.);
    sum_a += (f32::from(c[3]) * -10.) + (f32::from(c[5]) * 10.);
    sum_a += (f32::from(c[6]) * -03.) + (f32::from(c[7]) * 30.);

    // matrix
    // -03,-10,-03,
    //   0,  0,  0,
    //  03, 10, 03
    let mut sum_b = 0.0;
    sum_b += (f32::from(c[0]) * -03.) + (f32::from(c[1]) * -10.);
    sum_b += (f32::from(c[2]) * -03.) + (f32::from(c[6]) * 03.);
    sum_b += (f32::from(c[7]) * 10.) + (f32::from(c[8]) * 03.);

    T::from_f32(((sum_a * sum_a) + (sum_b * sum_b)).sqrt())
}

/// Calculate scharr for int  images
///
/// # Arguments
/// -  in_values: An array which is expected to contain 9 elements 
///  that represents a 3x3 window for which we are to calculate the sobel values
#[allow(clippy::neg_multiply, clippy::identity_op, clippy::zero_prefixed_literal)]
#[rustfmt::skip]
fn scharr_inner_i32<T>(c: &[T; 9]) -> T
    where
        T: NumOps<T> + Copy + Default,
        i32: std::convert::From<T>
{
    // Gx matrix
    //   -3, 0,  3,
    //  -10, 0, 10,
    //   -3, 0,  3
    //
    let mut sum_a = 0;
    sum_a += (i32::from(c[0]) * -03) + (i32::from(c[2]) * 03);
    sum_a += (i32::from(c[3]) * -10) + (i32::from(c[5]) * 10);
    sum_a += (i32::from(c[6]) * -03) + (i32::from(c[7]) * 03);

    // Gy matrix
    // -3,-10,-3,
    //  0,  0, 0,
    //  3, 10, 3
    let mut sum_b = 0;
    sum_b += (i32::from(c[0]) * -03) + (i32::from(c[1]) * -10);
    sum_b += (i32::from(c[2]) * -03) + (i32::from(c[6]) * 03);
    sum_b += (i32::from(c[7]) * 10) + (i32::from(c[8]) * 03);

    T::from_f64(f64::from((sum_a * sum_a) + (sum_b * sum_b)).sqrt())
}

/// Carry out the scharr filter for a float channel
///
/// Uses float operations hence may be slower than the equivalent integer functions
///
/// # Arguments.
/// - in_channel: Input channel for which contains image coefficients
/// - out_channel: Output channel for which we will fill with new sobel coefficients
/// - width: Width of input channel
/// - height: Height of input channel
pub fn scharr_float<T>(in_channel: &[T], out_channel: &mut [T], width: usize, height: usize)
where
    T: Default + NumOps<T> + Copy,
    f32: std::convert::From<T>
{
    //pad here
    let padded_input = pad(in_channel, width, height, 1, 1, PadMethod::Replicate);

    spatial_NxN::<_, _, 1, 9>(&padded_input, out_channel, width, height, scharr_inner_f32);
}

/// Carry out the scharr filter for an integer channel
///
/// Uses integer operations hence may be faster than the equivalent float functions
///
/// # Arguments.
/// - in_channel: Input channel for which contains image coefficients
/// - out_channel: Output channel for which we will fill with new sobel coefficients
/// - width: Width of input channel
/// - height: Height of input channel
pub fn scharr_int<T>(in_channel: &[T], out_channel: &mut [T], width: usize, height: usize)
where
    T: Default + NumOps<T> + Copy,
    i32: std::convert::From<T>
{
    //pad here
    let padded_input = pad(in_channel, width, height, 1, 1, PadMethod::Replicate);

    spatial_NxN::<_, _, 1, 9>(&padded_input, out_channel, width, height, scharr_inner_i32);
}
