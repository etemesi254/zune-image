#![cfg(feature = "x86")]
#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![allow(clippy::module_name_repetitions, clippy::wildcard_imports)]

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::convert::TryInto;

#[inline]
pub fn upsample_horizontal_sse(
    input: &[i16], in_ref: &mut [i16], scratch_space: &mut [i16], output: &mut [i16],
)
{
    unsafe { upsample_horizontal_sse_u(input, in_ref, scratch_space, output) }
}

/// Upsample using SSE to improve speed
///
/// The sampling filter is bi-linear or triangle filter
#[target_feature(enable = "sse2")]
//Some things are weird...
#[target_feature(enable = "sse4.1")]
#[inline]
pub unsafe fn upsample_horizontal_sse_u(
    input: &[i16], _in_ref: &mut [i16], _scratch_space: &mut [i16], out: &mut [i16],
)
{
    // set first 8 pixels linearly
    // Assert that out has more than 8 elements and input has more than 4
    // Do this before otherwise Rust will bounds check all of these items like some
    // paranoid guy.
    assert!(out.len() > 8 && input.len() > 5);

    //@ OPTIMIZE TIP: Borrow slices of a definite length and turn them into a reference
    // array to eliminate bounds checking.
    let f_out: &mut [i16; 8] = out.get_mut(0..8).unwrap().try_into().unwrap();

    // We can do better here, since Rust loads input[y] twice but it can store it in
    // a register but enough ugly code
    f_out[0] = input[0];

    f_out[1] = (input[0] * 3 + input[1] + 2) >> 2;

    f_out[2] = (input[1] * 3 + input[0] + 2) >> 2;

    f_out[3] = (input[1] * 3 + input[2] + 2) >> 2;

    f_out[4] = (input[2] * 3 + input[1] + 2) >> 2;

    f_out[5] = (input[2] * 3 + input[3] + 2) >> 2;

    f_out[6] = (input[3] * 3 + input[2] + 2) >> 2;

    f_out[7] = (input[3] * 3 + input[4] + 2) >> 2;

    // maths
    // The poop here is to calculate how many 8 items we can interleave without
    // reading past the array We can't do the last 8 using SSE, because we will
    // read after, and neither can we read from the first 8 because
    // we'll underflow in the first array so those are handled differently
    let inl = input.len();

    // times we can iterate without going past array;

    // For the rest of the pixels use normal instructions
    // Process using SSE for as many times as we can

    for i in 1..(inl >> 2) - 1
    {
        let pos = i << 2;

        let mut yn = _mm_loadl_epi64(input.get_unchecked(pos..).as_ptr().cast());

        yn = _mm_unpacklo_epi16(yn, yn); //[a,a,b,b,c,c,d,d]

        let v = _mm_loadl_epi64(input.get_unchecked(pos - 1..).as_ptr().cast());

        let y = _mm_loadl_epi64(input.get_unchecked(pos + 1..).as_ptr().cast());

        let even = _mm_unpacklo_epi16(v, v); //[a,a,b,b,c,c,d,d]

        let odd = _mm_unpacklo_epi16(y, y); //[ e,e,f,f,g,g,g.h,h]

        let nn = _mm_blend_epi16::<0b1010_1010>(even, odd); // [a,e,b,f,c,g,d,h]

        // a multiplication by 3 can be seen as a shift by 1 and add by itself, let's
        // use that to reduce latency

        // input[x]*3
        // Change multiplication by 3 to be a shift left by 1(multiplication by 2) and
        // add, removes latency arising from multiplication, but it seems RUST
        // is straight up ignoring me and my (cool) optimization techniques
        // it has converted it to a multiplication  RUST WHY DON'T YOU TRUST ME...
        let an = _mm_add_epi16(_mm_slli_epi16::<1>(yn), yn);

        // hoping this favours ILP because they don't depend on each other?
        let bn = _mm_add_epi16(nn, _mm_set1_epi16(2));

        // (input[x]*3+input[y]+2)>>2;
        let cn = _mm_srai_epi16::<2>(_mm_add_epi16(an, bn));

        // write to array
        _mm_storeu_si128(
            out.get_mut(i * 8..(i * 8) + 8).unwrap().as_mut_ptr().cast(),
            cn,
        );
    }

    // Do the last 8 manually because we can't  do it  with SSE because of out of bounds access
    let ol = out.len() - 8;

    let il = input.len() - 4;

    let l_out: &mut [i16; 8] = out.get_mut(ol..ol + 8).unwrap().try_into().unwrap();

    l_out[0] = (input[il] * 3 + input[il - 1] + 2) >> 2;

    l_out[1] = (input[il] * 3 + input[il + 1] + 2) >> 2;

    l_out[2] = (input[il + 1] * 3 + input[il] + 2) >> 2;

    l_out[3] = (input[il + 1] * 3 + input[il + 1] + 2) >> 2;

    l_out[4] = (input[il + 2] * 3 + input[il + 2] + 2) >> 2;

    l_out[5] = (input[il + 2] * 3 + input[il + 1] + 2) >> 2;

    l_out[6] = (input[il + 2] * 3 + input[il + 3] + 2) >> 2;

    l_out[7] = input[il + 3];
}
