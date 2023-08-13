/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "avx2")]

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use crate::grayscale::scalar::convert_rgb_to_grayscale_scalar;

#[target_feature(enable = "avx2")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub(crate) unsafe fn convert_rgb_to_grayscale_u8_avx2(r: &[u8], g: &[u8], b: &[u8], gr: &mut [u8]) {
    // Code is from https://stackoverflow.com/questions/57832444/efficient-c-code-no-libs-for-image-transformation-into-custom-rgb-pixel-grey
    // Code is from https://stackoverflow.com/questions/57832444/efficient-c-code-no-libs-for-image-transformation-into-custom-rgb-pixel-grey
    const CHUNK_SIZE: usize = 16;
    // Each coefficient is expanded by 2^15, and rounded to int16 (add 0.5 for rounding).
    let r_coef = _mm256_set1_epi16((0.2989 * 32768.0 + 0.5) as i16); //8 coefficients - R scale factor.
    let g_coef = _mm256_set1_epi16((0.5870 * 32768.0 + 0.5) as i16); //8 coefficients - G scale factor.
    let b_coef = _mm256_set1_epi16((0.1140 * 32768.0 + 0.5) as i16); //8 coefficients - B scale factor.

    for (((r_chunk, g_chunk), b_chunk), out) in r
        .chunks_exact(CHUNK_SIZE)
        .zip(b.chunks_exact(CHUNK_SIZE))
        .zip(g.chunks_exact(CHUNK_SIZE))
        .zip(gr.chunks_exact_mut(CHUNK_SIZE))
    {
        // Load to memory
        let mut r_c = _mm256_cvtepu8_epi16(_mm_loadu_si128(r_chunk.as_ptr().cast()));
        let mut g_c = _mm256_cvtepu8_epi16(_mm_loadu_si128(g_chunk.as_ptr().cast()));
        let mut b_c = _mm256_cvtepu8_epi16(_mm_loadu_si128(b_chunk.as_ptr().cast()));

        // Multiply input elements by 64 for improved accuracy.
        r_c = _mm256_slli_epi16::<6>(r_c);
        g_c = _mm256_slli_epi16::<6>(g_c);
        b_c = _mm256_slli_epi16::<6>(b_c);

        //Use the special intrinsic _mm256_mulhrs_epi16 that calculates round((r * r_coef)>>15)
        //Calculate Y = 0.2989*R + 0.5870*G + 0.1140*B (using fixed point computations)
        let mut g_out = _mm256_add_epi16(
            _mm256_add_epi16(
                _mm256_mulhrs_epi16(r_c, r_coef),
                _mm256_mulhrs_epi16(g_c, g_coef)
            ),
            _mm256_mulhrs_epi16(b_c, b_coef)
        );

        // Undo the multiplication
        g_out = _mm256_srli_epi16::<6>(g_out);
        // Pack 16 bits into 8 bits
        // [a0-a7] -> [g0,g1,g2,g3..g0,g1,g2,g3,..g8,g9,g10,g11...g8,g9,g10,g11]
        g_out = _mm256_packus_epi16(g_out, g_out);
        // we want to get ([a0-a8] + [a16-a24] ) (nb also [a8-a16] + [a24-a31] can achieve this.
        // Into either the lower or the higher register
        // So we can use permute instructions, i.e the one that manipulates
        // 64 bit integers(we can see that packus generates 64 bit packed from
        // each register, according to the docs), so let's unpack them and rearrange g_out
        // this can be destructive to the higher registers, but we really do not care
        g_out = _mm256_permute4x64_epi64::<0b00_00_11_00>(g_out);

        // Write out
        _mm_storeu_si128(
            out.as_mut_ptr().cast(),
            _mm256_extracti128_si256::<0>(g_out)
        );
    }
    // remainders
    if r.len() % CHUNK_SIZE != 0 {
        // do the remainder
        let rem = r.len() % CHUNK_SIZE;
        let start = r.len() - rem;
        let c_start = r.len() - (rem / 3);

        let c1 = &r[c_start..];
        let c2 = &g[c_start..];
        let c3 = &b[c_start..];

        convert_rgb_to_grayscale_scalar(c1, c2, c3, &mut gr[start..], 255);
    }
}
