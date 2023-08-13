/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "simd")]

use crate::deinterleave::scalar::{
    de_interleave_four_channels_scalar, de_interleave_three_channels_scalar
};

#[target_feature(enable = "avx2")]
pub(crate) unsafe fn de_interleave_three_channels_avx2<T: Copy>(
    source: &[T], c1: &mut [T], c2: &mut [T], c3: &mut [T]
) {
    // Rely on the auto-vectorizer
    // it does a decent work, i.e https://godbolt.org/z/W8brrdv4K
    // and I'm too lazy to figure out mine.
    de_interleave_three_channels_scalar(source, c1, c2, c3);
}

#[target_feature(enable = "avx2")]
pub(crate) unsafe fn de_interleave_four_channels_avx2<T: Copy>(
    source: &[T], c1: &mut [T], c2: &mut [T], c3: &mut [T], c4: &mut [T]
) {
    // Rely on the auto-vectorizer
    // it does a decent work, i.e https://godbolt.org/z/W8brrdv4K
    // and I'm too lazy to figure out mine.
    de_interleave_four_channels_scalar(source, c1, c2, c3, c4);
}
