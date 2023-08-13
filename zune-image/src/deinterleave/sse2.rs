/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "simd")]

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use crate::deinterleave::scalar::de_interleave_three_channels_scalar;

/// De-interleave 3 channel data to different channels
///
/// # Operation
/// ```text
/// [r,g,b,r,g,b] ->
/// [r,r],[g,g],[b,b]
/// ```
#[target_feature(enable = "sse2")]
pub(crate) unsafe fn de_interleave_three_channels_sse2(
    source: &[u8], c1: &mut [u8], c2: &mut [u8], c3: &mut [u8]
) {
    const CHUNK_SIZE: usize = 96;
    const OUT_CHUNK_SIZE: usize = CHUNK_SIZE / 3;

    assert_eq!(source.len() % 3, 0, "Source must be divisible by 3");
    assert_eq!(c1.len(), c2.len(), "Out sources must be of equal size");
    assert_eq!(c2.len(), c3.len(), "Out sources must be of equal size");

    for (((source_chunk, a), b), c) in source
        .chunks_exact(CHUNK_SIZE)
        .zip(c1.chunks_exact_mut(OUT_CHUNK_SIZE))
        .zip(c2.chunks_exact_mut(OUT_CHUNK_SIZE))
        .zip(c3.chunks_exact_mut(OUT_CHUNK_SIZE))
    {
        // https://docs.google.com/presentation/d/1I0-SiHid1hTsv7tjLST2dYW5YF5AJVfs9l4Rg9rvz48/htmlpresent
        let layer0_chunk0 = _mm_loadu_si128(source_chunk.as_ptr().cast());
        let layer0_chunk1 = _mm_loadu_si128(source_chunk[16..].as_ptr().cast());
        let layer0_chunk2 = _mm_loadu_si128(source_chunk[32..].as_ptr().cast());
        let layer0_chunk3 = _mm_loadu_si128(source_chunk[48..].as_ptr().cast());
        let layer0_chunk4 = _mm_loadu_si128(source_chunk[64..].as_ptr().cast());
        let layer0_chunk5 = _mm_loadu_si128(source_chunk[80..].as_ptr().cast());

        let layer1_chunk0 = _mm_unpacklo_epi8(layer0_chunk0, layer0_chunk3);
        let layer1_chunk1 = _mm_unpackhi_epi8(layer0_chunk0, layer0_chunk3);
        let layer1_chunk2 = _mm_unpacklo_epi8(layer0_chunk1, layer0_chunk4);
        let layer1_chunk3 = _mm_unpackhi_epi8(layer0_chunk1, layer0_chunk4);
        let layer1_chunk4 = _mm_unpacklo_epi8(layer0_chunk2, layer0_chunk5);
        let layer1_chunk5 = _mm_unpackhi_epi8(layer0_chunk2, layer0_chunk5);

        let layer2_chunk0 = _mm_unpacklo_epi8(layer1_chunk0, layer1_chunk3);
        let layer2_chunk1 = _mm_unpackhi_epi8(layer1_chunk0, layer1_chunk3);
        let layer2_chunk2 = _mm_unpacklo_epi8(layer1_chunk1, layer1_chunk4);
        let layer2_chunk3 = _mm_unpackhi_epi8(layer1_chunk1, layer1_chunk4);
        let layer2_chunk4 = _mm_unpacklo_epi8(layer1_chunk2, layer1_chunk5);
        let layer2_chunk5 = _mm_unpackhi_epi8(layer1_chunk2, layer1_chunk5);

        let layer3_chunk0 = _mm_unpacklo_epi8(layer2_chunk0, layer2_chunk3);
        let layer3_chunk1 = _mm_unpackhi_epi8(layer2_chunk0, layer2_chunk3);
        let layer3_chunk2 = _mm_unpacklo_epi8(layer2_chunk1, layer2_chunk4);
        let layer3_chunk3 = _mm_unpackhi_epi8(layer2_chunk1, layer2_chunk4);
        let layer3_chunk4 = _mm_unpacklo_epi8(layer2_chunk2, layer2_chunk5);
        let layer3_chunk5 = _mm_unpackhi_epi8(layer2_chunk2, layer2_chunk5);

        let layer4_chunk0 = _mm_unpacklo_epi8(layer3_chunk0, layer3_chunk3);
        let layer4_chunk1 = _mm_unpackhi_epi8(layer3_chunk0, layer3_chunk3);
        let layer4_chunk2 = _mm_unpacklo_epi8(layer3_chunk1, layer3_chunk4);
        let layer4_chunk3 = _mm_unpackhi_epi8(layer3_chunk1, layer3_chunk4);
        let layer4_chunk4 = _mm_unpacklo_epi8(layer3_chunk2, layer3_chunk5);
        let layer4_chunk5 = _mm_unpackhi_epi8(layer3_chunk2, layer3_chunk5);

        let red_chunk0 = _mm_unpacklo_epi8(layer4_chunk0, layer4_chunk3);
        let red_chunk1 = _mm_unpackhi_epi8(layer4_chunk0, layer4_chunk3);

        let green_chunk0 = _mm_unpacklo_epi8(layer4_chunk1, layer4_chunk4);
        let green_chunk1 = _mm_unpackhi_epi8(layer4_chunk1, layer4_chunk4);

        let blue_chunk0 = _mm_unpacklo_epi8(layer4_chunk2, layer4_chunk5);
        let blue_chunk1 = _mm_unpackhi_epi8(layer4_chunk2, layer4_chunk5);

        // convert to u16s
        _mm_storeu_si128(a[00..].as_mut_ptr().cast(), red_chunk0);
        _mm_storeu_si128(a[16..].as_mut_ptr().cast(), red_chunk1);
        _mm_storeu_si128(b[00..].as_mut_ptr().cast(), green_chunk0);
        _mm_storeu_si128(b[16..].as_mut_ptr().cast(), green_chunk1);
        _mm_storeu_si128(c[00..].as_mut_ptr().cast(), blue_chunk0);
        _mm_storeu_si128(c[16..].as_mut_ptr().cast(), blue_chunk1);
    }
    if source.len() % CHUNK_SIZE != 0 {
        // do the remainder
        let rem = source.len() % CHUNK_SIZE;
        let start = source.len() - rem;
        let c_start = c1.len() - (rem / 3);

        let c1 = &mut c1[c_start..];
        let c2 = &mut c2[c_start..];
        let c3 = &mut c3[c_start..];

        de_interleave_three_channels_scalar(&source[start..], c1, c2, c3);
    }
}
