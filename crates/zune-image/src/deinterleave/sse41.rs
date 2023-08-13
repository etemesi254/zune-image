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

use crate::deinterleave::scalar::{
    de_interleave_four_channels_scalar, de_interleave_three_channels_scalar
};

#[target_feature(enable = "sse4.1")]
pub unsafe fn de_interleave_three_channels_sse3_u8(
    source: &[u8], c1: &mut [u8], c2: &mut [u8], c3: &mut [u8]
) {
    const CHUNK_SIZE: usize = 48;
    const OUT_CHUNK_SIZE: usize = CHUNK_SIZE / 3;

    assert_eq!(source.len() % 3, 0, "Source must be divisible by 3");
    assert_eq!(c1.len(), c2.len(), "Out sources must be of equal size");
    assert_eq!(c2.len(), c3.len(), "Out sources must be of equal size");

    let ssse3_red_indeces_0 =
        _mm_set_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 15, 12, 9, 6, 3, 0);
    let ssse3_red_indeces_1 =
        _mm_set_epi8(-1, -1, -1, -1, -1, 14, 11, 8, 5, 2, -1, -1, -1, -1, -1, -1);
    let ssse3_red_indeces_2 =
        _mm_set_epi8(13, 10, 7, 4, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1);

    let ssse3_green_indeces_0 =
        _mm_set_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 13, 10, 7, 4, 1);
    let ssse3_green_indeces_1 =
        _mm_set_epi8(-1, -1, -1, -1, -1, 15, 12, 9, 6, 3, 0, -1, -1, -1, -1, -1);
    let ssse3_green_indeces_2 =
        _mm_set_epi8(14, 11, 8, 5, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1);

    let ssse3_blue_indeces_0 =
        _mm_set_epi8(-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 14, 11, 8, 5, 2);
    let ssse3_blue_indeces_1 =
        _mm_set_epi8(-1, -1, -1, -1, -1, -1, 13, 10, 7, 4, 1, -1, -1, -1, -1, -1);
    let ssse3_blue_indeces_2 =
        _mm_set_epi8(15, 12, 9, 6, 3, 0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1);

    for (((source_chunk, a), b), c) in source
        .chunks_exact(CHUNK_SIZE)
        .zip(c1.chunks_exact_mut(OUT_CHUNK_SIZE))
        .zip(c2.chunks_exact_mut(OUT_CHUNK_SIZE))
        .zip(c3.chunks_exact_mut(OUT_CHUNK_SIZE))
    {
        // https://docs.google.com/presentation/d/1I0-SiHid1hTsv7tjLST2dYW5YF5AJVfs9l4Rg9rvz48/htmlpresent

        let chunk0 = _mm_loadu_si128(source_chunk.as_ptr().cast());
        let chunk1 = _mm_loadu_si128(source_chunk[16..].as_ptr().cast());
        let chunk2 = _mm_loadu_si128(source_chunk[32..].as_ptr().cast());

        let red = _mm_or_si128(
            _mm_or_si128(
                _mm_shuffle_epi8(chunk0, ssse3_red_indeces_0),
                _mm_shuffle_epi8(chunk1, ssse3_red_indeces_1)
            ),
            _mm_shuffle_epi8(chunk2, ssse3_red_indeces_2)
        );
        let green = _mm_or_si128(
            _mm_or_si128(
                _mm_shuffle_epi8(chunk0, ssse3_green_indeces_0),
                _mm_shuffle_epi8(chunk1, ssse3_green_indeces_1)
            ),
            _mm_shuffle_epi8(chunk2, ssse3_green_indeces_2)
        );

        let blue = _mm_or_si128(
            _mm_or_si128(
                _mm_shuffle_epi8(chunk0, ssse3_blue_indeces_0),
                _mm_shuffle_epi8(chunk1, ssse3_blue_indeces_1)
            ),
            _mm_shuffle_epi8(chunk2, ssse3_blue_indeces_2)
        );

        _mm_storeu_si128(a.as_mut_ptr().cast(), red);
        _mm_storeu_si128(b.as_mut_ptr().cast(), green);
        _mm_storeu_si128(c.as_mut_ptr().cast(), blue);
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

#[allow(clippy::zero_prefixed_literal)]
#[target_feature(enable = "sse4.1")]
#[rustfmt::skip]
pub unsafe fn de_interleave_four_channels_sse41(
    source: &[u8], c1: &mut [u8], c2: &mut [u8], c3: &mut [u8], c4: &mut [u8],
)
{
    // https://godbolt.org/z/hhTarvMds

    const CHUNK_SIZE: usize = 64;
    const OUT_CHUNK_SIZE: usize = CHUNK_SIZE / 4;

    let shuffle_mask = _mm_set_epi8(15, 11, 7, 3,
                                    14, 10, 6, 2,
                                    13, 09, 5, 1,
                                    12, 08, 4, 0);

    for ((((source_chunk, rr), gg), bb), aa) in source
        .chunks_exact(CHUNK_SIZE)
        .zip(c1.chunks_exact_mut(OUT_CHUNK_SIZE))
        .zip(c2.chunks_exact_mut(OUT_CHUNK_SIZE))
        .zip(c3.chunks_exact_mut(OUT_CHUNK_SIZE))
        .zip(c4.chunks_exact_mut(OUT_CHUNK_SIZE))
    {
        // Data is in [r,g,b,a,r,g,b,r,g,b,a,r,g,b,a]
        //

        // We load 64 bytes to ensure that when we write, we do a write of 16 which
        // fits nicely into a sse register.
        let t1 = _mm_loadu_si128(source_chunk[00..].as_ptr().cast());
        let t2 = _mm_loadu_si128(source_chunk[08..].as_ptr().cast());
        let t3 = _mm_loadu_si128(source_chunk[16..].as_ptr().cast());
        let t4 = _mm_loadu_si128(source_chunk[24..].as_ptr().cast());

        // convert data into
        // rrrr,gggg,bbbb,aaaa
        let x1 = _mm_shuffle_epi8(t1, shuffle_mask);
        let x2 = _mm_shuffle_epi8(t2, shuffle_mask);
        let x3 = _mm_shuffle_epi8(t3, shuffle_mask);
        let x4 = _mm_shuffle_epi8(t4, shuffle_mask);

        // Each register contains rrrr,gggg bbbb and aaaa
        // so now bring them all together. so that a register has
        // rrrrrrrrr,ggggggg,bbbbbb and aaaaaa

        // rrrrrr,gggggg
        let p1 = _mm_unpacklo_epi32(x1, x2);
        // rrrrr,gggggg
        let p2 = _mm_unpacklo_epi32(x3, x4);
        // bbbbb,aaaaaa
        let p3 = _mm_unpackhi_epi32(x1, x2);
        // bbbbb,aaaaaa
        let p4 = _mm_unpackhi_epi32(x3, x4);

        let rrrr = _mm_unpacklo_epi64(p1, p2);
        let gggg = _mm_unpackhi_epi64(p1, p2);
        let bbbb = _mm_unpacklo_epi64(p3, p4);
        let aaaa = _mm_unpackhi_epi64(p3, p4);

        _mm_storeu_si128(rr.as_mut_ptr().cast(), rrrr);
        _mm_storeu_si128(gg.as_mut_ptr().cast(), gggg);
        _mm_storeu_si128(bb.as_mut_ptr().cast(), bbbb);
        _mm_storeu_si128(aa.as_mut_ptr().cast(), aaaa);
    }
    if source.len() % CHUNK_SIZE != 0
    {
        // do the remainder
        let rem = source.len() % CHUNK_SIZE;
        let start = source.len() - rem;
        let c_start = c1.len() - (rem / 4);

        let c1 = &mut c1[c_start..];
        let c2 = &mut c2[c_start..];
        let c3 = &mut c3[c_start..];
        let c4 = &mut c4[c_start..];

        de_interleave_four_channels_scalar(&source[start..], c1, c2, c3, c4);
    }
}
