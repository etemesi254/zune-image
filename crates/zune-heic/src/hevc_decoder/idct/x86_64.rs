#![cfg(target_arch = "x86_64")]
#![allow(unreachable_code)]

use core::arch::x86_64::*;

const DC_VERTICAL_SCALE: i32 = 64;

// ---------------------------------------------------------------------------
// Constants: Basis Matrices — 64-byte aligned for cache-line friendliness
// ---------------------------------------------------------------------------

#[repr(align(64))]
struct AlignedBasis4([[i16; 4]; 4]);
#[repr(align(64))]
struct AlignedBasis8([[i16; 8]; 8]);
#[repr(align(64))]
struct AlignedBasis16([[i16; 16]; 16]);

#[rustfmt::skip]
static T8: AlignedBasis4 = AlignedBasis4([
    [89,  75,  50,  18], [75, -18, -89, -50],
    [50, -89,  18,  75], [18, -50,  75, -89],
]);

#[rustfmt::skip]
static T16: AlignedBasis8 = AlignedBasis8([
    [90,  87,  80,  70,  57,  43,  25,   9], [87,  57,   9, -43, -80, -90, -70, -25],
    [80,   9, -70, -87, -25,  57,  90,  43], [70, -43, -87,  -9,  90,  25, -80, -57],
    [57, -80, -25,  90,  -9, -87,  43,  70], [43, -90,  57,  25, -87,  70,   9, -80],
    [25, -70,  90, -80,  43,   9, -57,  87], [ 9, -25,  43, -57,  70, -80,  87, -90],
]);

#[rustfmt::skip]
static T32: AlignedBasis16 = AlignedBasis16([
    [90, 90, 88, 85, 82, 78, 73, 67, 61, 54, 46, 38, 31, 22, 13,  4],
    [90, 82, 67, 46, 22, -4,-31,-54,-73,-85,-90,-88,-78,-61,-38,-13],
    [88, 67, 31,-13,-54,-82,-90,-78,-46, -4, 38, 73, 90, 85, 61, 22],
    [85, 46,-13,-67,-90,-73,-22, 38, 82, 88, 54, -4,-61,-90,-78,-31],
    [82, 22,-54,-90,-61, 13, 78, 85, 31,-46,-90,-67,  4, 73, 88, 38],
    [78, -4,-82,-73, 13, 85, 67,-22,-88,-61, 31, 90, 54,-38,-90,-46],
    [73,-31,-90,-22, 78, 67,-38,-90,-13, 82, 61,-46,-88, -4, 85, 54],
    [67,-54,-78, 38, 85,-22,-90,  4, 90, 13,-88,-31, 82, 46,-73,-61],
    [61,-73,-46, 82, 31,-88,-13, 90, -4,-90, 22, 85,-38,-78, 54, 67],
    [54,-85, -4, 88,-46,-61, 82, 13,-90, 38, 67,-78,-22, 90,-31,-73],
    [46,-90, 38, 54,-90, 31, 61,-88, 22, 67,-85, 13, 73,-82,  4, 78],
    [38,-88, 73, -4,-67, 90,-46,-31, 85,-78, 13, 61,-90, 54, 22,-82],
    [31,-78, 90,-61,  4, 54,-88, 82,-38,-22, 73,-90, 67,-13,-46, 85],
    [22,-61, 85,-90, 73,-38, -4, 46,-78, 90,-82, 54,-13,-31, 67,-88],
    [13,-38, 61,-78, 88,-90, 85,-73, 54,-31,  4, 22,-46, 67,-82, 90],
    [ 4,-13, 22,-31, 38,-46, 54,-61, 67,-73, 78,-82, 85,-88, 90,-90],
]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
fn shift_clip(val: i32, shift: i32) -> i16 {
    let offset = (1i32 << shift) >> 1;
    ((val + offset) >> shift).clamp(-32768, 32767) as i16
}

#[inline(always)]
unsafe fn shift_clip_x4(vals: &[i32; 4], shift: i32) -> [i16; 4] {
    let v = _mm_loadu_si128(vals.as_ptr() as *const __m128i);
    let offset = _mm_set1_epi32((1 << shift) >> 1);
    let shifted = _mm_srai_epi32(_mm_add_epi32(v, offset), shift);
    let packed = _mm_packs_epi32(shifted, shifted);
    let mut out = [0i16; 4];
    _mm_storel_epi64(out.as_mut_ptr() as *mut __m128i, packed);
    out
}

#[inline(always)]
unsafe fn shift_clip_x8(vals: &[i32; 8], shift: i32) -> [i16; 8] {
    let lo = _mm_loadu_si128(vals.as_ptr() as *const __m128i);
    let hi = _mm_loadu_si128(vals.as_ptr().add(4) as *const __m128i);
    let offset = _mm_set1_epi32((1 << shift) >> 1);

    let slo = _mm_srai_epi32(_mm_add_epi32(lo, offset), shift);
    let shi = _mm_srai_epi32(_mm_add_epi32(hi, offset), shift);

    let packed = _mm_packs_epi32(slo, shi);
    let mut out = [0i16; 8];
    _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, packed);
    out
}

#[inline(always)]
unsafe fn shift_clip_x16(vals: &[i32; 16], shift: i32) -> [i16; 16] {
    let mut out = [0i16; 16];
    let o1 = shift_clip_x8(vals[0..8].try_into().unwrap(), shift);
    let o2 = shift_clip_x8(vals[8..16].try_into().unwrap(), shift);
    out[0..8].copy_from_slice(&o1);
    out[8..16].copy_from_slice(&o2);
    out
}

#[inline(always)]
unsafe fn shift_clip_x32(vals: &[i32; 32], shift: i32) -> [i16; 32] {
    let mut out = [0i16; 32];
    let o1 = shift_clip_x16(vals[0..16].try_into().unwrap(), shift);
    let o2 = shift_clip_x16(vals[16..32].try_into().unwrap(), shift);
    out[0..16].copy_from_slice(&o1);
    out[16..32].copy_from_slice(&o2);
    out
}

#[inline(always)]
fn shift_clip_slice<const N: usize>(vals: &[i32; 32], shift: i32, out: &mut [i16]) {
    unsafe {
        match N {
            4 => out[..4].copy_from_slice(&shift_clip_x4(vals[..4].try_into().unwrap(), shift)),
            8 => out[..8].copy_from_slice(&shift_clip_x8(vals[..8].try_into().unwrap(), shift)),
            16 => out[..16].copy_from_slice(&shift_clip_x16(vals[..16].try_into().unwrap(), shift)),
            32 => out[..32].copy_from_slice(&shift_clip_x32(vals[..32].try_into().unwrap(), shift)),
            _ => unreachable!(),
        }
    }
}

#[inline(always)]
unsafe fn is_all_zero_impl(s: &[i16]) -> bool {
    let mut chunks = s.chunks_exact(8);
    let mut acc = _mm_setzero_si128();
    for chunk in chunks.by_ref() {
        let v = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
        acc = _mm_or_si128(acc, v);
    }
    let cmp = _mm_cmpeq_epi8(acc, _mm_setzero_si128());
    let mask = _mm_movemask_epi8(cmp);
    if mask != 0xFFFF {
        return false;
    }
    chunks.remainder().iter().all(|&v| v == 0)
}

#[inline(always)]
fn is_all_zero(s: &[i16]) -> bool {
    unsafe { is_all_zero_impl(s) }
}

#[inline(always)]
unsafe fn hsum_epi32(m: __m128i) -> i32 {
    let t1 = _mm_add_epi32(m, _mm_shuffle_epi32(m, 0x4E));
    let t2 = _mm_add_epi32(t1, _mm_shuffle_epi32(t1, 0xB1));
    _mm_cvtsi128_si32(t2)
}

#[inline(always)]
unsafe fn dot4(a: &[i16; 4], b: &[i16; 4]) -> i32 {
    let va = _mm_loadl_epi64(a.as_ptr() as *const __m128i);
    let vb = _mm_loadl_epi64(b.as_ptr() as *const __m128i);
    let m = _mm_madd_epi16(va, vb);
    hsum_epi32(m)
}

#[inline(always)]
unsafe fn dot8(a: &[i16; 8], b: &[i16; 8]) -> i32 {
    let va = _mm_loadu_si128(a.as_ptr() as *const __m128i);
    let vb = _mm_loadu_si128(b.as_ptr() as *const __m128i);
    let m = _mm_madd_epi16(va, vb);
    hsum_epi32(m)
}

#[inline(always)]
unsafe fn dot16(a: &[i16; 16], b: &[i16; 16]) -> i32 {
    let va1 = _mm_loadu_si128(a.as_ptr() as *const __m128i);
    let vb1 = _mm_loadu_si128(b.as_ptr() as *const __m128i);
    let m1 = _mm_madd_epi16(va1, vb1);

    let va2 = _mm_loadu_si128(a.as_ptr().add(8) as *const __m128i);
    let vb2 = _mm_loadu_si128(b.as_ptr().add(8) as *const __m128i);
    let m2 = _mm_madd_epi16(va2, vb2);

    let m = _mm_add_epi32(m1, m2);
    hsum_epi32(m)
}

// ---------------------------------------------------------------------------
// 1D Kernels
// ---------------------------------------------------------------------------

#[inline(always)]
fn transform4_dst_1d_simd(input: &[i16], output: &mut [i32]) {
    unsafe {
        let va = _mm_loadl_epi64(input.as_ptr() as *const __m128i);
        let m0 = _mm_madd_epi16(va, _mm_set_epi16(0, 0, 0, 0, 55, 84, 74, 29));
        let m1 = _mm_madd_epi16(va, _mm_set_epi16(0, 0, 0, 0, -84, -29, 74, 55));
        let m2 = _mm_madd_epi16(va, _mm_set_epi16(0, 0, 0, 0, 74, -74, 0, 74));
        let m3 = _mm_madd_epi16(va, _mm_set_epi16(0, 0, 0, 0, -29, 55, -74, 84));

        output[0] = hsum_epi32(m0);
        output[1] = hsum_epi32(m1);
        output[2] = hsum_epi32(m2);
        output[3] = hsum_epi32(m3);
    }
}

#[inline(always)]
fn transform4_1d_simd(input: &[i16], output: &mut [i32]) {
    let c0 = i32::from(input[0]) * 64;
    let c2 = i32::from(input[2]) * 64;
    let c1 = i32::from(input[1]);
    let c3 = i32::from(input[3]);

    let o0 = c1 * 83 + c3 * 36;
    let o1 = c1 * 36 - c3 * 83;

    unsafe {
        let e = _mm_set_epi32(c0 + c2, c0 - c2, c0 - c2, c0 + c2);
        let o = _mm_set_epi32(-o0, -o1, o1, o0);
        let res = _mm_add_epi32(e, o);
        _mm_storeu_si128(output.as_mut_ptr() as *mut __m128i, res);
    }
}

#[inline(always)]
fn transform8_1d_simd(input: &[i16], output: &mut [i32]) {
    let ee0 = (i32::from(input[0]) * 64) + (i32::from(input[4]) * 64);
    let ee1 = (i32::from(input[0]) * 64) - (i32::from(input[4]) * 64);
    let eo0 = (i32::from(input[2]) * 83) + (i32::from(input[6]) * 36);
    let eo1 = (i32::from(input[2]) * 36) - (i32::from(input[6]) * 83);
    let e = [ee0 + eo0, ee1 + eo1, ee1 - eo1, ee0 - eo0];

    let odd_in = [input[1], input[3], input[5], input[7]];
    let mut o = [0i32; 4];
    for i in 0..4 {
        unsafe {
            o[i] = dot4(&odd_in, &T8.0[i]);
        }
    }

    for i in 0..4 {
        output[i] = e[i] + o[i];
        output[7 - i] = e[i] - o[i];
    }
}

#[inline(always)]
fn transform16_1d_simd(input: &[i16], output: &mut [i32]) {
    let mut even_in = [0i16; 8];
    for i in 0..8 {
        even_in[i] = input[i * 2];
    }

    let mut e = [0i32; 8];
    transform8_1d_simd(&even_in, &mut e);

    let odd_in = [
        input[1], input[3], input[5], input[7], input[9], input[11], input[13], input[15]
    ];

    let mut o = [0i32; 8];
    for i in 0..8 {
        unsafe {
            o[i] = dot8(&odd_in, &T16.0[i]);
        }
    }

    for i in 0..8 {
        output[i] = e[i] + o[i];
        output[15 - i] = e[i] - o[i];
    }
}

#[inline(always)]
fn transform32_1d_simd(input: &[i16], output: &mut [i32]) {
    let mut even_in = [0i16; 16];
    for i in 0..16 {
        even_in[i] = input[i * 2];
    }

    let mut e = [0i32; 16];
    transform16_1d_simd(&even_in, &mut e);

    let odd_in = [
        input[1], input[3], input[5], input[7], input[9], input[11], input[13], input[15],
        input[17], input[19], input[21], input[23], input[25], input[27], input[29], input[31]
    ];

    let mut o = [0i32; 16];
    for i in 0..16 {
        unsafe {
            o[i] = dot16(&odd_in, &T32.0[i]);
        }
    }

    for i in 0..16 {
        output[i] = e[i] + o[i];
        output[31 - i] = e[i] - o[i];
    }
}

// ---------------------------------------------------------------------------
// 2D Wrapper
// ---------------------------------------------------------------------------

pub fn idct_2d_core<const N: usize>(
    block: &mut [i16], intermediate: &mut [i16; 1024], bit_depth: u8, is_dst: bool,
    transform_1d: fn(&[i16], &mut [i32])
) {
    let shift1: i32 = 7;
    let shift2: i32 = 20 - i32::from(bit_depth);

    for c in 0..N {
        let mut col_in = [0i16; 32];
        for r in 0..N {
            col_in[r] = block[r * N + c];
        }

        if is_all_zero(&col_in[..N]) {
            for r in 0..N {
                intermediate[r * N + c] = 0;
            }
            continue;
        }

        if !is_dst && col_in[0] != 0 && is_all_zero(&col_in[1..N]) {
            let dc_val = shift_clip(i32::from(col_in[0]) * 64, shift1);
            for r in 0..N {
                intermediate[r * N + c] = dc_val;
            }
            continue;
        }

        let mut col_out = [0i32; 32];
        transform_1d(&col_in[..N], &mut col_out);

        let mut clipped = [0i16; 32];
        shift_clip_slice::<N>(&col_out, shift1, &mut clipped);
        for r in 0..N {
            intermediate[r * N + c] = clipped[r];
        }
    }

    for r in 0..N {
        let row = &intermediate[r * N..r * N + N];

        if is_all_zero(row) {
            for c in 0..N {
                block[r * N + c] = 0;
            }
            continue;
        }

        if !is_dst && row[0] != 0 && is_all_zero(&row[1..N]) {
            let val = shift_clip(i32::from(row[0]) * DC_VERTICAL_SCALE, shift2);
            for c in 0..N {
                block[r * N + c] = val;
            }
            continue;
        }

        let mut row_out = [0i32; 32];
        transform_1d(row, &mut row_out);

        let mut clipped = [0i16; 32];
        shift_clip_slice::<N>(&row_out, shift2, &mut clipped);
        for c in 0..N {
            block[r * N + c] = clipped[c];
        }
    }
}

// ---------------------------------------------------------------------------
// Entry Points
// ---------------------------------------------------------------------------

pub fn idst_4x4_hevc(block: &mut [i16; 16], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    idct_2d_core::<4>(block, scratchpad, bit_depth, true, transform4_dst_1d_simd);
}

pub fn idct_4x4_hevc(block: &mut [i16; 16], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    idct_2d_core::<4>(block, scratchpad, bit_depth, false, transform4_1d_simd);
}

pub fn idct_8x8_hevc(block: &mut [i16; 64], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    idct_2d_core::<8>(block, scratchpad, bit_depth, false, transform8_1d_simd);
}

pub fn idct_16x16_hevc(block: &mut [i16; 256], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    idct_2d_core::<16>(block, scratchpad, bit_depth, false, transform16_1d_simd);
}

pub fn idct_32x32_hevc(block: &mut [i16; 1024], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    idct_2d_core::<32>(block, scratchpad, bit_depth, false, transform32_1d_simd);
}