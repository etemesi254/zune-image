#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "avx2")]

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use crate::grayscale::scalar::convert_rgb_to_grayscale_scalar;

#[target_feature(enable = "avx2")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub(crate) unsafe fn convert_rgb_to_grayscale_avx2(
    r: &[u16], g: &[u16], b: &[u16], gr: &mut [u16], max_value: u16,
)
{
    // Code is from https://stackoverflow.com/questions/57832444/efficient-c-code-no-libs-for-image-transformation-into-custom-rgb-pixel-grey
    const CHUNK_SIZE: usize = 16;
    // Each coefficient is expanded by 2^15, and rounded to int16 (add 0.5 for rounding).
    let r_coef = _mm256_set1_epi16((0.2989 * 32768.0 + 0.5) as i16); //8 coefficients - R scale factor.
    let g_coef = _mm256_set1_epi16((0.5870 * 32768.0 + 0.5) as i16); //8 coefficients - G scale factor.
    let b_coef = _mm256_set1_epi16((0.1140 * 32768.0 + 0.5) as i16); //8 coefficients - B scale factor.

    let max_val = _mm256_set1_epi16(max_value as i16);

    for (((r_chunk, g_chunk), b_chunk), out) in r
        .chunks_exact(CHUNK_SIZE)
        .zip(b.chunks_exact(CHUNK_SIZE))
        .zip(g.chunks_exact(CHUNK_SIZE))
        .zip(gr.chunks_exact_mut(CHUNK_SIZE))
    {
        // Load to memory
        let mut r_c = _mm256_loadu_si256(r_chunk.as_ptr().cast());
        let mut g_c = _mm256_loadu_si256(g_chunk.as_ptr().cast());
        let mut b_c = _mm256_loadu_si256(b_chunk.as_ptr().cast());

        // Multiply input elements by 64 for improved accuracy.
        r_c = _mm256_slli_epi16::<6>(r_c);
        g_c = _mm256_slli_epi16::<6>(g_c);
        b_c = _mm256_slli_epi16::<6>(b_c);

        //Use the special intrinsic _mm256_mulhrs_epi16 that calculates round((r * r_coef)>>15)
        //Calculate Y = 0.2989*R + 0.5870*G + 0.1140*B (using fixed point computations)
        let mut g_out = _mm256_add_epi16(
            _mm256_add_epi16(
                _mm256_mulhrs_epi16(r_c, r_coef),
                _mm256_mulhrs_epi16(g_c, g_coef),
            ),
            _mm256_mulhrs_epi16(b_c, b_coef),
        );

        // Undo the multiplication
        g_out = _mm256_srli_epi16::<6>(g_out);

        // clamp
        g_out = _mm256_min_epu16(g_out, max_val);
        // store
        _mm256_storeu_si256(out.as_mut_ptr().cast(), g_out);
    }
    // remainders
    if r.len() % CHUNK_SIZE != 0
    {
        // do the remainder
        let rem = r.len() % CHUNK_SIZE;
        let start = r.len() - rem;
        let c_start = r.len() - (rem / 3);

        let c1 = &r[c_start..];
        let c2 = &g[c_start..];
        let c3 = &b[c_start..];

        convert_rgb_to_grayscale_scalar(c1, c2, c3, &mut gr[start..], max_value);
    }
}
