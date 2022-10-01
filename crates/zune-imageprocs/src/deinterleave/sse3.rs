#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "sse3")]

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use crate::deinterleave::scalar::de_interleave_3_channels_scalar;

#[target_feature(enable = "sse3")]
pub unsafe fn de_interleave_3_channels_sse3(
    source: &[u8], (c1, c2, c3): (&mut [u8], &mut [u8], &mut [u8]),
)
{
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
                _mm_shuffle_epi8(chunk1, ssse3_red_indeces_1),
            ),
            _mm_shuffle_epi8(chunk2, ssse3_red_indeces_2),
        );
        let green = _mm_or_si128(
            _mm_or_si128(
                _mm_shuffle_epi8(chunk0, ssse3_green_indeces_0),
                _mm_shuffle_epi8(chunk1, ssse3_green_indeces_1),
            ),
            _mm_shuffle_epi8(chunk2, ssse3_green_indeces_2),
        );

        let blue = _mm_or_si128(
            _mm_or_si128(
                _mm_shuffle_epi8(chunk0, ssse3_blue_indeces_0),
                _mm_shuffle_epi8(chunk1, ssse3_blue_indeces_1),
            ),
            _mm_shuffle_epi8(chunk2, ssse3_blue_indeces_2),
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

        de_interleave_3_channels_scalar(&source[start..], (c1, c2, c3));
    }
}
