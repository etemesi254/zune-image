/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Alpha pre-multiplication routines
//!
//! This module contains routines to convert to and from
//! premultiplied alpha
//!
//!
//!  # Algorithm
//! - To compute premultiplied alpha, we simply multiply alpha by color channel
//!  so assuming float to convert from straight alpha to premultiplied alpha we can use
//!  `RGB*a`, here `a` is between 0.0-1.0 (RGB are three color components, a multiplied
//!  them separately)
//!  
//! We can handle this in integers, e.g for u8, the mapping `0.0-`1.0` can be scaled by `255` to
//! get alpha value in u8, and then doing alpha-premultiplication can be presented by the formula
//!
//! - `(RGB*255)/a`, we first scale the value by 255 in order for division to work since now the mapping
//! is (255..65535), if your `R` value is 1 and alpha is 255, you get `(1*255)/255`, and similarly, if your value is `255`
//! and alpha is 128 you get `(255*255)/128`. This allows us to actually carry out alpha pre-multiplication in integers
//!
//! - But division is a slow instruction and Rust tends to add checks for zero (and appropriate panics)
//! hence it's either badly vectorized or not vectorized, furthermore it's hard to parallelize it
//! in the instruction cache pipeline, which means the simple operation reduces speed.
//!
//! #### A solution
//!
//!  - The solution here is that for integers, we are entirely in a fixed bounds, e.g for u8, we are
//! bounded by `0..255`, always, and u16, we are bound to `0..65535`, (PS this doesn't apply for floats)
//! and we can apply another optimization, namely Daniel's Lemire [fastmod](https://github.com/lemire/fastmod)
//! so to compute special constants during runtime that can be used to divide via reciprocal multiplication
//! so now to create premultiplied alpha, we simply the code becomes
//!
//! - Compute constants
//! - Iterate over source channel and alpha,
//! - Lookup special constant (`c` ) for the alpha value
//! - Multiply that constant `c` with channel value and take top bits
//! [`fastdiv_u32`](crate::mathops::fastdiv_u32)
//! -

use crate::mathops::{compute_mod_u32, fastdiv_u32};

mod sse;

/// Create the fastdiv table for u8 division
///
/// Useful for speeding up un-pre-multiplying alpha
#[allow(clippy::needless_range_loop)]
#[must_use]
pub fn create_unpremul_table_u8() -> [u128; 256] {
    let mut array = [0; 256];

    for i in 1..256 {
        array[i] = compute_mod_u32(i as u64);
    }

    array
}

/// Create a fastdiv table for u16 division
///
/// Useful for speeding up un-pre-multiplying alpha
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn create_unpremul_table_u16() -> Vec<u128> {
    let mut array = vec![0; 65536];

    for i in 1..65536 {
        array[i] = compute_mod_u32(i as u64);
    }

    array
}

///  Simple pre-multiply alpha implementation
///
/// # Arguments
///
/// * `input`:  Input array which contains non-premultiplied alpha
/// * `alpha`:  Alpha values corresponding to pre-multiplied alpha
///
/// Items in input are modified in place.
#[allow(clippy::cast_possible_truncation)]
pub fn premultiply_u8(input: &mut [u8], alpha: &[u8]) {
    const MAX_VALUE: u16 = 255;

    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        let temp = (u16::from(*al) * u16::from(*color)) + 0x80;

        *color = ((temp + (temp >> 8)) / MAX_VALUE) as u8;
    });
}

/// Specialized pre_multiply for u16
///
/// # Arguments
///
/// * `input`: Input array, values modified in place
/// * `alpha`: Alpha for associated values
///
/// returns: Array modified in place
#[allow(clippy::cast_possible_truncation)]
pub fn premultiply_u16(input: &mut [u16], alpha: &[u16]) {
    const MAX_VALUE: u32 = 65535;

    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        let temp = (u32::from(*al) * u32::from(*color)) + ((MAX_VALUE + 1) / 2);
        *color = ((temp + (temp >> 16)) / MAX_VALUE) as u16;
    });
}

/// Remove effects of pre-multiplied alpha
///
/// # Arguments
///
/// * `input` : Input u8 values which are pre-multiplied with alpha
/// * `alpha` : The alpha value pre-multiplied
pub fn unpremultiply_u8(input: &mut [u8], alpha: &[u8], premul_table: &[u128; 256]) {
    // we did         pa = (color * alpha)/255,
    // to undo we do  pb = (color * 255  )/alpha

    const MAX_VALUE: u32 = 255;

    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        let associated_alpha = premul_table[usize::from(*al)];
        *color = fastdiv_u32(
            u32::from(*color) * MAX_VALUE + (u32::from(*al) / 2),
            associated_alpha
        ) as u8;
    });
}

/// Undo effect of alpha multiplication
///
/// # Arguments
///
/// * `input`: Array with pre-multiplied coefficients
/// * `alpha`:  The alpha value which was pre-multiplied
/// * `premul_table`:  A table of coefficients which contains special
///   numbers that do reciprocal multiplication, generated by
///   [create_unpremul_table_u16](create_unpremul_table_u16)
///
/// Array is modified in place
pub fn unpremultiply_u16(input: &mut [u16], alpha: &[u16], premul_table: &[u128]) {
    // we did         pa = (color * alpha)/65535,
    // to undo we do  pb = (color * 65535)/alpha

    const MAX_VALUE: u32 = 65535;

    debug_assert!(premul_table.len() > 65535);
    if premul_table.len() < 65536 {
        // this invariant ensures that we remove bounds check from below loop
        // u16 range from 0..65535, hence premul_table[usize::from(u16)] will
        // always be in bounds if this is true
        return;
    }

    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        let associated_alpha = premul_table[usize::from(*al)];

        *color = fastdiv_u32(
            u32::from(*color) * MAX_VALUE + (u32::from(*al) / 2),
            associated_alpha
        ) as u16;
    });
}

/// Premultiply a f32 array with the associated alpha
///
/// # Arguments
///
/// * `input`: Input channel containing un-premultiplied alpha
/// * `alpha`: Alpha values for that channel
///
///
/// Array is modified in place
pub fn premultiply_f32(input: &mut [f32], alpha: &[f32]) {
    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        *color *= al;
    });
}

fn unpremultiply_f32_scalar(input: &mut [f32], alpha: &[f32]) {
    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        if *al == 0.0 {
            *color = 0.0;
        } else {
            // avoid div by zero
            *color /= *al;
        }
    });
}

/// Undo the effects of pre-multiplied alpha on the array
///
/// # Arguments
///
/// * `input`: Input data
/// * `alpha`: Alpha associated for the input data
///
/// # Behaviour
/// - When alpha channel is zero, input also becomes zero
///
/// Array is modified in place
pub fn unpremultiply_f32(input: &mut [f32], alpha: &[f32]) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "sse2")]
        {
            if is_x86_feature_detected!("sse") {
                return unsafe { crate::premul_alpha::sse::unpremultiply_sse_f32(input, alpha) };
            }
        }
    }
    unpremultiply_f32_scalar(input, alpha);
}
