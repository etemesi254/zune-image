#![allow(unreachable_code)]

use std::simd::cmp::{SimdOrd, SimdPartialEq};
use std::simd::num::SimdInt;
use std::simd::{LaneCount, Simd, SupportedLaneCount, i16x4, i16x8, i16x16, i32x4, i32x8, i32x16};

// ---------------------------------------------------------------------------
// Constants: Basis Matrices (Stored as i16 to save memory/cache)
// ---------------------------------------------------------------------------

#[rustfmt::skip]
const T8: [[i16; 4]; 4] = [
    [89,  75,  50,  18], [75, -18, -89, -50],
    [50, -89,  18,  75], [18, -50,  75, -89],
];

#[rustfmt::skip]
const T16: [[i16; 8]; 8] = [
    [90,  87,  80,  70,  57,  43,  25,   9], [87,  57,   9, -43, -80, -90, -70, -25],
    [80,   9, -70, -87, -25,  57,  90,  43], [70, -43, -87,  -9,  90,  25, -80, -57],
    [57, -80, -25,  90,  -9, -87,  43,  70], [43, -90,  57,  25, -87,  70,   9, -80],
    [25, -70,  90, -80,  43,   9, -57,  87], [ 9, -25,  43, -57,  70, -80,  87, -90],
];

#[rustfmt::skip]
const T32: [[i16; 16]; 16] = [
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
    [18,-50, 75,-89, 89,-75, 50,-18,-18, 50,-75, 89,-89, 75,-50, 18],
    [ 4,-13, 22,-31, 38,-46, 54,-61, 67,-73, 78,-82, 85,-88, 90,-90],
];

// ---------------------------------------------------------------------------
// 1D Kernels (Input i16 -> Output unclipped i32)
// ---------------------------------------------------------------------------

fn transform4_dst_1d_simd(input: &[i16], output: &mut [i32]) {
    // Load i16 and instantly promote to i32 for safe math!
    let v = i16x4::from_slice(&input[..4]).cast::<i32>();

    let c0 = i32x4::from_array([29, 55, 74, 84]);
    let c1 = i32x4::from_array([74, 74, 0, -74]);
    let c2 = i32x4::from_array([84, -29, -74, 55]);
    let c3 = i32x4::from_array([55, -84, 74, -29]);

    output[0] = (v * c0).reduce_sum();
    output[1] = (v * c1).reduce_sum();
    output[2] = (v * c2).reduce_sum();
    output[3] = (v * c3).reduce_sum();
}

fn transform4_1d_simd(input: &[i16], output: &mut [i32]) {
    let c0 = input[0] as i32;
    let c1 = input[1] as i32;
    let c2 = input[2] as i32;
    let c3 = input[3] as i32;

    let e0 = (c0 * 64) + (c2 * 64);
    let e1 = (c0 * 64) - (c2 * 64);
    let o0 = (c1 * 83) + (c3 * 36);
    let o1 = (c1 * 36) - (c3 * 83);

    output[0] = e0 + o0;
    output[1] = e1 + o1;
    output[2] = e1 - o1;
    output[3] = e0 - o0;
}

fn transform8_1d_simd(input: &[i16], output: &mut [i32]) {
    let ee0 = (input[0] as i32 * 64) + (input[4] as i32 * 64);
    let ee1 = (input[0] as i32 * 64) - (input[4] as i32 * 64);
    let eo0 = (input[2] as i32 * 83) + (input[6] as i32 * 36);
    let eo1 = (input[2] as i32 * 36) - (input[6] as i32 * 83);
    let e = [ee0 + eo0, ee1 + eo1, ee1 - eo1, ee0 - eo0];

    // SIMD for the Odd part
    let odd_in = i16x4::from_array([input[1], input[3], input[5], input[7]]).cast::<i32>();
    let mut o = [0i32; 4];
    for i in 0..4 {
        let basis = i16x4::from_array(T8[i]).cast::<i32>();
        o[i] = (odd_in * basis).reduce_sum();
    }

    for i in 0..4 {
        output[i] = e[i] + o[i];
        output[7 - i] = e[i] - o[i];
    }
}

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
        let basis = i16x8::from_array(T16[i]).cast::<i32>();
        o[i] = (odd_in * basis).reduce_sum();
    }

    for i in 0..8 {
        output[i] = e[i] + o[i];
        output[15 - i] = e[i] - o[i];
    }
}

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
        let basis = i16x16::from_array(T32[i]).cast::<i32>();
        o[i] = (odd_in * basis).reduce_sum();
    }

    for i in 0..16 {
        output[i] = e[i] + o[i];
        output[31 - i] = e[i] - o[i];
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline(always)]
fn shift_clip(val: i32, shift: i32) -> i16 {
    let offset = if shift > 0 { 1 << (shift - 1) } else { 0 };
    ((val + offset) >> shift).clamp(-32768, 32767) as i16
}

// ---------------------------------------------------------------------------
// 2D Wrapper
// ---------------------------------------------------------------------------

pub fn idct_2d_core<const N: usize>(
    block: &mut [i16], intermediate: &mut [i16; 1024], bit_depth: u8, is_dst: bool,
    transform_1d: fn(&[i16], &mut [i32])
) {
    let shift1 = 7;
    let shift2 = 20 - bit_depth as i32;

    // Pass 1: Horizontal
    for r in 0..N {
        let row_start = r * N;
        let row = &block[row_start..row_start + N];

        if row.iter().all(|&v| v == 0) {
            for c in 0..N {
                intermediate[c * N + r] = 0;
            }
            continue;
        }

        let dc_only = row[0] != 0 && row[1..N].iter().all(|&v| v == 0);

        if dc_only && !is_dst {
            let dc_val = shift_clip((row[0] as i32) * 64, shift1);
            for c in 0..N {
                intermediate[c * N + r] = dc_val;
            }
        } else {
            let mut row_out = [0i32; 32];
            transform_1d(row, &mut row_out);
            for c in 0..N {
                intermediate[c * N + r] = shift_clip(row_out[c], shift1);
            }
        }
    }

    // Pass 2: Vertical
    for r in 0..N {
        let col_start = r * N;
        let col = &intermediate[col_start..col_start + N];

        if col.iter().all(|&v| v == 0) {
            for c in 0..N {
                block[c * N + r] = 0;
            }
            continue;
        }

        let dc_only = col[0] != 0 && col[1..N].iter().all(|&v| v == 0);

        if dc_only && !is_dst {
            let val = shift_clip((col[0] as i32) * 64, shift2);
            for c in 0..N {
                block[c * N + r] = val;
            }
        } else {
            let mut col_out = [0i32; 32];
            transform_1d(col, &mut col_out);
            for c in 0..N {
                block[c * N + r] = shift_clip(col_out[c], shift2);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry Points (Expects i16 buffers)
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
