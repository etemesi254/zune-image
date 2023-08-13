/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#[inline(always)]
pub(crate) fn de_interleave_three_channels_scalar<T: Copy>(
    source: &[T], c1: &mut [T], c2: &mut [T], c3: &mut [T]
) {
    for (((val, a), b), c) in source
        .chunks_exact(3)
        .zip(c1.iter_mut())
        .zip(c2.iter_mut())
        .zip(c3.iter_mut())
    {
        *a = val[0];
        *b = val[1];
        *c = val[2];
    }
}

#[inline]
pub fn de_interleave_four_channels_scalar<T: Copy>(
    source: &[T], c1: &mut [T], c2: &mut [T], c3: &mut [T], c4: &mut [T]
) {
    for ((((src, c11), c22), c33), c44) in source
        .chunks_exact(4)
        .zip(c1.iter_mut())
        .zip(c2.iter_mut())
        .zip(c3.iter_mut())
        .zip(c4.iter_mut())
    {
        *c11 = src[0];
        *c22 = src[1];
        *c33 = src[2];
        *c44 = src[3];
    }
}
