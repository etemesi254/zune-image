/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::ops::Sub;

use crate::mathops::{compute_mod_u32, fastdiv_u32};
use crate::traits::NumOps;

///
/// Linearly stretches the contrast in an image in place,
/// sending lower to image minimum and upper to image maximum.
///
/// # Arguments
///
/// * `image`: Image channel pixels
/// * `lower`:  The lower minimum for which pixels below this value
///  become 0
/// * `upper`:  Upper maximum for which pixels above this value become the maximum
///  value
/// * `maximum`: Maximum value for this pixel type.
///
/// - Modifies array in place
///
pub fn stretch_contrast<T>(image: &mut [T], lower: T, upper: T, maximum: u32)
where
    T: Ord + Sub<Output = T> + NumOps<T> + Copy,
    u32: std::convert::From<T>
{
    assert!(upper > lower, "upper must be strictly greater than lower");

    let len = u32::from(upper.saturating_sub(lower)).saturating_add(1);

    // using fast division is faster than the other one
    // (weirdly) vectorized one.
    //
    // image dimensions: 5796 * 3984 RGB
    //  using the vectorizable one: 303 ms
    //  using fastdiv:              142 ms
    //
    // Probably due to better pipelining.
    let mod_len = compute_mod_u32(u64::from(len));

    for pixel in image.iter_mut() {
        if *pixel >= upper {
            *pixel = T::max_val();
        } else if *pixel <= lower {
            *pixel = T::min_val();
        } else {
            let numerator = maximum * u32::from(*pixel - lower);
            let scaled = fastdiv_u32(numerator, mod_len);
            *pixel = T::from_u32(scaled);
        }
    }
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use crate::stretch_contrast::stretch_contrast;

    #[bench]
    fn bench_stretch_contrast(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let mut in_vec = vec![255_u16; dimensions];

        b.iter(|| {
            stretch_contrast(&mut in_vec, 3, 10, 65535);
        });
    }
}
