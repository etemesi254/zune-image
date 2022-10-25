//! SSE color conversion routines
#![allow(
    clippy::module_name_repetitions,
    clippy::doc_markdown,
    clippy::wildcard_imports
)]
#![cfg(feature = "x86")]
#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::convert::TryInto;

union XmmRegister
{
    array: [i16; 8],
    reg:   __m128i,
}

pub fn ycbcr_to_rgb_sse(
    y: &[i16; 8], cb: &[i16; 8], cr: &[i16; 8], out: &mut [u8], offset: &mut usize,
)
{
    unsafe {
        ycbcr_to_rgb_sse41(y, cb, cr, out, offset);
    }
}

#[inline]
#[target_feature(enable = "sse4.1")]
unsafe fn ycbcr_to_rgb_sse41(
    y: &[i16; 8], cb: &[i16; 8], cr: &[i16; 8], out: &mut [u8], offset: &mut usize,
)
{
    out.get_mut(*offset..*offset + 24)
        .expect("Slice to small cannot write");

    // SSE can only store 4 i32's in a register
    // this means we either use two registers and carry calculations
    // which is wasteful(since the values are always clamped to 0..255)
    // so a solution is to load into to different registers and pack them into
    // one register, which is what we do here

    // Safety,
    // - This method is called with arrays of 8 vectors
    // - Also arrays are explicitly aligned to 32 byte boundaries otherwise
    //   _mm_load_si128 would
    // segfault
    let y = _mm_loadu_si128(y.as_ptr().cast());

    let cb = _mm_loadu_si128(cb.as_ptr().cast());

    let cr = _mm_loadu_si128(cr.as_ptr().cast());

    // SSE version of integer version in https://stackoverflow.com/questions/4041840/function-to-convert-ycbcr-to-rgb

    let cr_r = _mm_sub_epi16(cr, _mm_set1_epi16(128));

    let cb_r = _mm_sub_epi16(cb, _mm_set1_epi16(128));

    // Calculate Y->R
    // r = Y + 45 * Cr / 32
    // 45*cr
    let r1 = _mm_mullo_epi16(_mm_set1_epi16(45), cr_r);

    // r1>>5
    let r2 = _mm_srai_epi16::<5>(r1);

    //y+r2

    let r = XmmRegister {
        reg: clamp_sse(_mm_add_epi16(y, r2)),
    };

    // g = Y - (11 * Cb + 23 * Cr) / 32 ;

    // 11*cb
    let g1 = _mm_mullo_epi16(_mm_set1_epi16(11), cb_r);

    // 23*cr
    let g2 = _mm_mullo_epi16(_mm_set1_epi16(23), cr_r);

    //(11
    //(11 * Cb + 23 * Cr)
    let g3 = _mm_add_epi16(g1, g2);

    // (11 * Cb + 23 * Cr) / 32
    let g4 = _mm_srai_epi16::<5>(g3);

    // Y - (11 * Cb + 23 * Cr) / 32 ;
    let g = XmmRegister {
        reg: clamp_sse(_mm_sub_epi16(y, g4)),
    };

    // b = Y + 113 * Cb / 64 ;
    // 113 * cb
    let b1 = _mm_mullo_epi16(_mm_set1_epi16(113), cb_r);

    //113 * Cb / 64
    let b2 = _mm_srai_epi16::<6>(b1);

    // b = Y + 113 * Cb / 64 ;
    let b = XmmRegister {
        reg: clamp_sse(_mm_add_epi16(b2, y)),
    };

    let pos = offset;

    // We add items to the array in trivial order
    // though thanks to the awesome technology that is RUST and LLVM, it is still
    // vectorised with some cool blend and broadcast instructions
    for i in 0..8
    {
        // Reason
        //  - Bounds checking will prevent autovectorization of this
        // Safety
        // - Array is pre initialized and the way this is called ensures
        // it will never go out op bounds
        *out.get_unchecked_mut(*pos) = r.array[i] as u8;

        *out.get_unchecked_mut(*pos + 1) = g.array[i] as u8;

        *out.get_unchecked_mut(*pos + 2) = b.array[i] as u8;

        *pos += 3;
    }
}

unsafe fn ycbcr_to_rgb_ax_sse41<const X: i16>(
    y: &[i16; 8], cb: &[i16; 8], cr: &[i16; 8], out: &mut [u8], offset: &mut usize,
)
{
    // SSE can only store 4 i32's in a register
    // this means we either use two registers and carry calculations
    // which is wasteful(since the values are always clamped to 0..255)
    // so a solution is to load into to different registers and pack them into
    // one register, which is what we do here

    let y = _mm_loadu_si128(y.as_ptr().cast());

    let cb = _mm_loadu_si128(cb.as_ptr().cast());

    let cr = _mm_loadu_si128(cr.as_ptr().cast());

    // SSE version of integer version in https://stackoverflow.com/questions/4041840/function-to-convert-ycbcr-to-rgb
    let cr_r = _mm_sub_epi16(cr, _mm_set1_epi16(128));

    let cb_r = _mm_sub_epi16(cb, _mm_set1_epi16(128));

    // Calculate Y->R
    // r = Y + 45 * Cr / 32
    // 45*cr
    let r1 = _mm_mullo_epi16(_mm_set1_epi16(45), cr_r);

    // r1>>5
    let r2 = _mm_srai_epi16::<5>(r1);

    //y+r2

    let r = _mm_add_epi16(y, r2);

    // g = Y - (11 * Cb + 23 * Cr) / 32 ;

    // 11*cb
    let g1 = _mm_mullo_epi16(_mm_set1_epi16(11), cb_r);

    // 23*cr
    let g2 = _mm_mullo_epi16(_mm_set1_epi16(23), cr_r);

    //(11
    //(11 * Cb + 23 * Cr)
    let g3 = _mm_add_epi16(g1, g2);

    // (11 * Cb + 23 * Cr) / 32
    let g4 = _mm_srai_epi16::<5>(g3);

    // Y - (11 * Cb + 23 * Cr) / 32 ;
    let g = _mm_sub_epi16(y, g4);

    // b = Y + 113 * Cb / 64 ;
    // 113 * cb
    let b1 = _mm_mullo_epi16(_mm_set1_epi16(113), cb_r);

    //113 * Cb / 64
    let b2 = _mm_srai_epi16::<6>(b1);

    // b = Y + 113 * Cb / 64 ;
    let b = _mm_add_epi16(b2, y);

    // We dont need to clamp for SSE, the packus instruction will do that for us

    let e = _mm_packus_epi16(r, g);

    let f = _mm_packus_epi16(b, _mm_set1_epi16(X));

    let g = _mm_unpacklo_epi8(e, f);

    let h = _mm_unpackhi_epi8(e, f);

    let i = _mm_unpacklo_epi8(g, h);

    let j = _mm_unpackhi_epi8(g, h);

    _mm_storeu_si128(out.as_mut_ptr().add(*offset).cast(), i);

    _mm_storeu_si128(out.as_mut_ptr().add(*offset + 16).cast(), j);

    *offset += 32;
}

/// Clamp using SSE
///
/// This shelves off about 16 instructions per conversion.
///
/// # Arguments
/// a: A mutable reference to a memory location containing
/// 4 i32's aligned to a 16 byte boundary.
///
/// The data is modified in place
#[inline]
#[target_feature(enable = "sse2")]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]

unsafe fn clamp_sse(a: __m128i) -> __m128i
{
    // the lowest value
    let min: __m128i = _mm_set1_epi16(0);

    // Highest value
    let max: __m128i = _mm_set1_epi16(255);

    let max_v = _mm_max_epi16(a, min); //max(a,0)
    let min_v = _mm_min_epi16(max_v, max); //min(max(a,0),255)
                                           // Store it back to the array
    return min_v;
}

/// Carry out YCbCr to RGB conversion to
/// emulate the avx version
#[inline]
#[target_feature(enable = "sse2")]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]

unsafe fn ycbcr_to_rgb_16(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize,
)
{
    {
        ycbcr_to_rgb_sse(
            y[0..8].try_into().unwrap(),
            cb[0..8].try_into().unwrap(),
            cr[0..8].try_into().unwrap(),
            out,
            offset,
        );

        // second MCU
        ycbcr_to_rgb_sse(
            y[8..16].try_into().unwrap(),
            cb[8..16].try_into().unwrap(),
            cr[8..16].try_into().unwrap(),
            out,
            offset,
        );
    }
}

pub fn ycbcr_to_rgb_sse_16(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize,
)
{
    // check if out has enough space
    out.get_mut(*offset..*offset + 64)
        .expect("Slice to small cannot write");
    unsafe {
        ycbcr_to_rgb_16(y, cb, cr, out, offset);
    }
}

pub fn ycbcr_to_rgba_sse_16(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize,
)
{
    // check if out has enough space
    out.get_mut(*offset..*offset + 64)
        .expect("Slice to small cannot write");
    unsafe {
        // not so random he he
        // first mcu
        ycbcr_to_rgb_ax_sse41::<255>(
            y[0..8].try_into().unwrap(),
            cb[0..8].try_into().unwrap(),
            cr[0..8].try_into().unwrap(),
            out,
            offset,
        );

        // second MCU
        ycbcr_to_rgb_ax_sse41::<255>(
            y[8..16].try_into().unwrap(),
            cb[8..16].try_into().unwrap(),
            cr[8..16].try_into().unwrap(),
            out,
            offset,
        );
    }
}
