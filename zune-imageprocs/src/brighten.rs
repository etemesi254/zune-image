/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Brighten operation
//!
//! # Algorithm
//!
//! The brightness algorithm is implemented as
//!
//! ```rust
//! x = a + c
//! ```
//!
//! where
//! - `x`: New pixel
//! - `a`: Old pixel
//! - `c`: How bright to increase the pixel
//!
//! The `a+c` is saturating on the maximum value for the type
//!
use crate::traits::NumOps;

/// Brighten operation
///
/// # Arguments
///
/// * `channel`: Input channel pixel, operates in place
/// * `value`: Value to increase the channel values with
/// * `_max_value`:  Currently unused
///
/// returns: ()
///
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn brighten<T: Copy + PartialOrd + NumOps<T> + Default>(
    channel: &mut [T], value: T, _max_value: T
)
{
    channel
        .iter_mut()
        .for_each(|x| *x = (*x).saturating_add(value));
}

pub fn brighten_f32(channel: &mut [f32], value: f32, max_value: f32)
{
    channel
        .iter_mut()
        .for_each(|x| *x = (*x + value).clamp(0.0, max_value));
}
