#![allow(unreachable_code)]
#![cfg(feature = "simd")]
use core::simd::cmp::SimdOrd;
use core::simd::num::SimdInt;
use core::simd::{i16x4, i16x8, i16x16, i32x4, i32x8};

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
    // Branchless: (1 << shift) >> 1 == 0 when shift == 0
    let offset = (1i32 << shift) >> 1;
    ((val + offset) >> shift).clamp(-32768, 32767) as i16
}
#[inline(always)]
fn shift_clip_x4(vals: &[i32; 4], shift: i32) -> [i16; 4] {
    let offset = i32x4::splat((1i32 << shift) >> 1);
    let shifted = (i32x4::from_array(*vals) + offset) >> i32x4::splat(shift);
    shifted
        .cast::<i16>()
        .simd_clamp(i16x4::splat(-32768), i16x4::splat(32767))
        .to_array()
}

#[inline(always)]
fn shift_clip_x8(vals: &[i32; 8], shift: i32) -> [i16; 8] {
    // i32x8 -> two clamped i16x4 lanes, merged
    let offset = i32x8::splat((1i32 << shift) >> 1);
    let shifted = (i32x8::from_array(*vals) + offset) >> i32x8::splat(shift);
    // No direct i32x8->i16x8 cast in std::simd without nightly truncate,
    // so split into two x4 lanes
    let lo: [i32; 4] = shifted.as_array()[0..4].try_into().unwrap();
    let hi: [i32; 4] = shifted.as_array()[4..8].try_into().unwrap();
    let lo16 = shift_clip_x4_raw(lo);
    let hi16 = shift_clip_x4_raw(hi);
    [
        lo16[0], lo16[1], lo16[2], lo16[3], hi16[0], hi16[1], hi16[2], hi16[3]
    ]
}

#[inline(always)]
fn shift_clip_x16(vals: &[i32; 16], shift: i32) -> [i16; 16] {
    let offset = (1i32 << shift) >> 1;
    let mut out = [0i16; 16];
    // Four x4 SIMD lanes across the 16 elements
    for chunk in 0..4 {
        let base = chunk * 4;
        let v = i32x4::from_slice(&vals[base..base + 4]);
        let s = (v + i32x4::splat(offset)) >> i32x4::splat(shift);
        let c = s
            .cast::<i16>()
            .simd_clamp(i16x4::splat(-32768), i16x4::splat(32767));
        out[base..base + 4].copy_from_slice(&c.to_array());
    }
    out
}

#[inline(always)]
fn shift_clip_x32(vals: &[i32; 32], shift: i32) -> [i16; 32] {
    let offset = (1i32 << shift) >> 1;
    let mut out = [0i16; 32];
    for chunk in 0..8 {
        let base = chunk * 4;
        let v = i32x4::from_slice(&vals[base..base + 4]);
        let s = (v + i32x4::splat(offset)) >> i32x4::splat(shift);
        let c = s
            .cast::<i16>()
            .simd_clamp(i16x4::splat(-32768), i16x4::splat(32767));
        out[base..base + 4].copy_from_slice(&c.to_array());
    }
    out
}

// Raw helper used by x8 — takes already-shifted i32x4, clamps to i16x4
#[inline(always)]
fn shift_clip_x4_raw(vals: [i32; 4]) -> [i16; 4] {
    i32x4::from_array(vals)
        .cast::<i16>()
        .simd_clamp(i16x4::splat(-32768), i16x4::splat(32767))
        .to_array()
}

// ---------------------------------------------------------------------------
// Generic shift_clip_slice — dispatches to the right SIMD width
// ---------------------------------------------------------------------------

#[inline(always)]
fn shift_clip_slice<const N: usize>(vals: &[i32; 32], shift: i32, out: &mut [i16]) {
    match N {
        4 => out[..4].copy_from_slice(&shift_clip_x4(vals[..4].try_into().unwrap(), shift)),
        8 => out[..8].copy_from_slice(&shift_clip_x8(vals[..8].try_into().unwrap(), shift)),
        16 => out[..16].copy_from_slice(&shift_clip_x16(vals[..16].try_into().unwrap(), shift)),
        32 => out[..32].copy_from_slice(&shift_clip_x32(vals[..32].try_into().unwrap(), shift)),
        _ => unreachable!()
    }
}

/// SIMD zero-check over an i16 slice. Chunks that don't fill a full i16x8
/// are checked scalarly. Much faster than .iter().all() on sparse blocks.
#[inline(always)]
fn is_all_zero(s: &[i16]) -> bool {
    let mut chunks = s.chunks_exact(8);
    let any_nonzero = chunks
        .by_ref()
        .map(i16x8::from_slice)
        .fold(i16x8::splat(0), |acc, v| acc | v);

    if any_nonzero != i16x8::splat(0) {
        return false;
    }
    chunks.remainder().iter().all(|&v| v == 0)
}

// ---------------------------------------------------------------------------
// 1D Kernels
// ---------------------------------------------------------------------------

#[inline(always)]
fn transform4_dst_1d_simd(input: &[i16], output: &mut [i32]) {
    let v = i16x4::from_slice(&input[..4]).cast::<i32>();

    // Corrected HEVC DST-VII transposed matrix (T^T * v)
    let c0 = i32x4::from_array([29, 74, 84, 55]);
    let c1 = i32x4::from_array([55, 74, -29, -84]);
    let c2 = i32x4::from_array([74, 0, -74, 74]);
    let c3 = i32x4::from_array([84, -74, 55, -29]);

    output[0] = (v * c0).reduce_sum();
    output[1] = (v * c1).reduce_sum();
    output[2] = (v * c2).reduce_sum();
    output[3] = (v * c3).reduce_sum();
}

#[inline(always)]
fn transform4_1d_simd(input: &[i16], output: &mut [i32]) {
    let c0 = i32::from(input[0]) * 64;
    let c2 = i32::from(input[2]) * 64;
    let c1 = i32::from(input[1]);
    let c3 = i32::from(input[3]);

    let o0 = c1 * 83 + c3 * 36;
    let o1 = c1 * 36 - c3 * 83;

    // Pack even and odd into SIMD vectors and add/sub in one shot
    let e = i32x4::from_array([c0 + c2, c0 - c2, c0 - c2, c0 + c2]);
    let o = i32x4::from_array([o0, o1, -o1, -o0]);
    (e + o).copy_to_slice(output);
}

#[inline(always)]
fn transform8_1d_simd(input: &[i16], output: &mut [i32]) {
    let ee0 = (i32::from(input[0]) * 64) + (i32::from(input[4]) * 64);
    let ee1 = (i32::from(input[0]) * 64) - (i32::from(input[4]) * 64);
    let eo0 = (i32::from(input[2]) * 83) + (i32::from(input[6]) * 36);
    let eo1 = (i32::from(input[2]) * 36) - (i32::from(input[6]) * 83);
    let e = [ee0 + eo0, ee1 + eo1, ee1 - eo1, ee0 - eo0];

    let odd_in = i16x4::from_array([input[1], input[3], input[5], input[7]]).cast::<i32>();
    let mut o = [0i32; 4];
    for i in 0..4 {
        let basis = i16x4::from_array(T8.0[i]).cast::<i32>();
        o[i] = (odd_in * basis).reduce_sum();
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

    let odd_in = i16x8::from_array([
        input[1], input[3], input[5], input[7], input[9], input[11], input[13], input[15]
    ])
    .cast::<i32>();

    let mut o = [0i32; 8];
    for i in 0..8 {
        let basis = i16x8::from_array(T16.0[i]).cast::<i32>();
        o[i] = (odd_in * basis).reduce_sum();
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

    let odd_in = i16x16::from_array([
        input[1], input[3], input[5], input[7], input[9], input[11], input[13], input[15],
        input[17], input[19], input[21], input[23], input[25], input[27], input[29], input[31]
    ])
    .cast::<i32>();

    let mut o = [0i32; 16];
    for i in 0..16 {
        let basis = i16x16::from_array(T32.0[i]).cast::<i32>();
        o[i] = (odd_in * basis).reduce_sum();
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

    // --- Pass 1: Vertical (columns of block → intermediate) -----------------
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

        // SIMD shift+clip the whole column in one vectorised sweep
        let mut clipped = [0i16; 32];
        shift_clip_slice::<N>(&col_out, shift1, &mut clipped);
        for r in 0..N {
            intermediate[r * N + c] = clipped[r];
        }
    }

    // --- Pass 2: Horizontal (rows of intermediate → block) ------------------
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

        // SIMD shift+clip directly into block
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
