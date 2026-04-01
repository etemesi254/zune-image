#![cfg(feature = "simd")]

use std::simd::cmp::{SimdOrd, SimdPartialEq};
use std::simd::num::SimdInt;
use std::simd::{Simd, i32x4, i32x8, i32x16};
// ---------------------------------------------------------------------------
// Constants: Basis Matrices (Odd Parts)
// ---------------------------------------------------------------------------
#[rustfmt::skip]
const T8: [[i32; 4]; 4] = [
    [89,  75,  50,  18], [75, -18, -89, -50],
    [50, -89,  18,  75], [18, -50,  75, -89],
];

#[rustfmt::skip]
const T16: [[i32; 8]; 8] = [
    [90,  87,  80,  70,  57,  43,  25,   9], [87,  57,   9, -43, -80, -90, -70, -25],
    [80,   9, -70, -87, -25,  57,  90,  43], [70, -43, -87,  -9,  90,  25, -80, -57],
    [57, -80, -25,  90,  -9, -87,  43,  70], [43, -90,  57,  25, -87,  70,   9, -80],
    [25, -70,  90, -80,  43,   9, -57,  87], [ 9, -25,  43, -57,  70, -80,  87, -90],
];

#[rustfmt::skip]
const T32: [[i32; 16]; 16] = [
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
// SIMD Helpers & Clipping
// ---------------------------------------------------------------------------

#[inline(always)]
fn simd_shift_clip<const LANES: usize>(val: Simd<i32, LANES>, shift: i32) -> Simd<i32, LANES>
where
    Simd<i32, LANES>:
        std::ops::Add<Output = Simd<i32, LANES>> + std::ops::Shr<i32, Output = Simd<i32, LANES>>
{
    let offset = if shift > 0 { 1 << (shift - 1) } else { 0 };
    (val + Simd::splat(offset)) >> (shift as i32)
}

#[inline(always)]
fn intermediate_clip<const LANES: usize>(val: Simd<i32, LANES>) -> Simd<i32, LANES> {
    val.simd_clamp(Simd::splat(-32768), Simd::splat(32767))
}

#[inline(always)]
fn final_clip<const LANES: usize>(val: Simd<i32, LANES>, bit_depth: u8) -> Simd<i32, LANES> {
    let max = (1 << bit_depth) - 1;
    val.simd_clamp(Simd::splat(0), Simd::splat(max))
}

#[inline(always)]
fn reverse_vec8(v: i32x8) -> i32x8 {
    Simd::from_array([v[7], v[6], v[5], v[4], v[3], v[2], v[1], v[0]])
}

#[inline(always)]
fn reverse_vec16(v: i32x16) -> i32x16 {
    Simd::from_array([
        v[15], v[14], v[13], v[12], v[11], v[10], v[9], v[8], v[7], v[6], v[5], v[4], v[3], v[2],
        v[1], v[0]
    ])
}

// ---------------------------------------------------------------------------
// 1D SIMD Transform Kernels
// ---------------------------------------------------------------------------

fn transform4_1d_simd(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let vec = i32x4::from_slice(input);
    let e0 = (vec[0] * 64) + (vec[2] * 64);
    let e1 = (vec[0] * 64) - (vec[2] * 64);
    let o0 = (vec[1] * 83) + (vec[3] * 36);
    let o1 = (vec[1] * 36) - (vec[3] * 83);

    let res = i32x4::from_array([e0 + o0, e1 + o1, e1 - o1, e0 - o0]);
    let v = simd_shift_clip(res, shift);
    let final_v = if should_clip { intermediate_clip(v) } else { v };
    output.copy_from_slice(final_v.as_array());
}

fn transform4_dst_1d_simd(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let v = i32x4::from_slice(input);
    let s0 = (v[0] * 29) + (v[1] * 55) + (v[2] * 74) + (v[3] * 84);
    let s1 = (v[0] * 74) + (v[1] * 74) - (v[3] * 74);
    let s2 = (v[0] * 84) - (v[1] * 29) - (v[2] * 74) + (v[3] * 55);
    let s3 = (v[0] * 55) - (v[1] * 84) + (v[2] * 74) - (v[3] * 29);

    let res = i32x4::from_array([s0, s1, s2, s3]);
    let v_res = simd_shift_clip(res, shift);
    let final_v = if should_clip { intermediate_clip(v_res) } else { v_res };
    output.copy_from_slice(final_v.as_array());
}

fn transform8_1d_simd(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let ee0 = (input[0] * 64) + (input[4] * 64);
    let ee1 = (input[0] * 64) - (input[4] * 64);
    let eo0 = (input[2] * 83) + (input[6] * 36);
    let eo1 = (input[2] * 36) - (input[6] * 83);
    let e = i32x4::from_array([ee0 + eo0, ee1 + eo1, ee1 - eo1, ee0 - eo0]);

    let odd_in = i32x4::from_array([input[1], input[3], input[5], input[7]]);
    let mut o = [0i32; 4];
    for i in 0..4 {
        o[i] = (odd_in * i32x4::from_array(T8[i])).reduce_sum();
    }
    let o_vec = i32x4::from_array(o);

    let v_first = simd_shift_clip(e + o_vec, shift);
    let v_last = simd_shift_clip(
        Simd::from_array([e[3], e[2], e[1], e[0]])
            - Simd::from_array([o_vec[3], o_vec[2], o_vec[1], o_vec[0]]),
        shift
    );

    let res_h = if should_clip { intermediate_clip(v_first) } else { v_first };
    let res_l = if should_clip { intermediate_clip(v_last) } else { v_last };

    output[0..4].copy_from_slice(res_h.as_array());
    output[4..8].copy_from_slice(res_l.as_array());
}

fn transform16_1d_simd(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let mut e = [0i32; 8];
    transform8_1d_simd(
        &[
            input[0], input[2], input[4], input[6], input[8], input[10], input[12], input[14]
        ],
        &mut e,
        0,
        false
    );

    let odd_in = i32x8::from_array([
        input[1], input[3], input[5], input[7], input[9], input[11], input[13], input[15]
    ]);

    let mut o_arr = [0i32; 8];
    for i in 0..8 {
        o_arr[i] = (odd_in * i32x8::from_array(T16[i])).reduce_sum();
    }

    let e_vec = i32x8::from_slice(&e);
    let o_vec = i32x8::from_array(o_arr);

    let v_first = simd_shift_clip(e_vec + o_vec, shift);
    let v_last = simd_shift_clip(reverse_vec8(e_vec) - reverse_vec8(o_vec), shift);

    let res_h = if should_clip { intermediate_clip(v_first) } else { v_first };
    let res_l = if should_clip { intermediate_clip(v_last) } else { v_last };

    output[0..8].copy_from_slice(res_h.as_array());
    output[8..16].copy_from_slice(res_l.as_array());
}

fn transform32_1d_simd(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let mut e = [0i32; 16];
    let mut even_in = [0i32; 16];
    for i in 0..16 {
        even_in[i] = input[i * 2];
    }
    transform16_1d_simd(&even_in, &mut e, 0, false);

    let odd_in = i32x16::from_array([
        input[1], input[3], input[5], input[7], input[9], input[11], input[13], input[15],
        input[17], input[19], input[21], input[23], input[25], input[27], input[29], input[31]
    ]);

    let mut o_arr = [0i32; 16];
    for i in 0..16 {
        o_arr[i] = (odd_in * i32x16::from_array(T32[i])).reduce_sum();
    }

    let e_vec = i32x16::from_slice(&e);
    let o_vec = i32x16::from_array(o_arr);

    let v_first = simd_shift_clip(e_vec + o_vec, shift);
    let v_last = simd_shift_clip(reverse_vec16(e_vec) - reverse_vec16(o_vec), shift);

    let res_h = if should_clip { intermediate_clip(v_first) } else { v_first };
    let res_l = if should_clip { intermediate_clip(v_last) } else { v_last };

    output[0..16].copy_from_slice(res_h.as_array());
    output[16..32].copy_from_slice(res_l.as_array());
}

// ---------------------------------------------------------------------------
// 2D Wrapper with All Optimizations
// ---------------------------------------------------------------------------

pub fn idct_2d_core<const N: usize>(
    block: &mut [i32], bit_depth: u8, is_dst: bool, transform_1d: fn(&[i32], &mut [i32], i32, bool)
) {
    let mut intermediate = [0i32; 1024];
    let shift1 = 7;
    let shift2 = 20 - bit_depth as i32;

    // Pass 1: Horizontal
    for r in 0..N {
        let row_start = r * N;
        let row = &block[row_start..row_start + N];

        // LNZ Early Exit
        let mut all_zero = true;
        for &val in row {
            if val != 0 {
                all_zero = false;
                break;
            }
        }
        if all_zero {
            for c in 0..N {
                intermediate[c * N + r] = 0;
            }
            continue;
        }

        let mut row_out = [0i32; 32];
        // Bit-exact DC shortcut
        let mut dc_only = row[0] != 0;
        for i in 1..N {
            if row[i] != 0 {
                dc_only = false;
                break;
            }
        }

        if dc_only && !is_dst {
            let dc_val = (row[0] * 64 + (1 << (shift1 - 1))) >> shift1;
            let clipped = dc_val.clamp(-32768, 32767);
            for c in 0..N {
                row_out[c] = clipped;
            }
        } else {
            transform_1d(row, &mut row_out[..N], shift1, true);
        }

        for c in 0..N {
            intermediate[c * N + r] = row_out[c];
        }
    }

    // Pass 2: Vertical
    for r in 0..N {
        let col_start = r * N;
        let col = &intermediate[col_start..col_start + N];

        let mut all_zero = true;
        for &val in col {
            if val != 0 {
                all_zero = false;
                break;
            }
        }
        if all_zero {
            for c in 0..N {
                block[c * N + r] = 0;
            }
            continue;
        }

        let mut col_out = [0i32; 32];
        let mut dc_only = col[0] != 0;
        for i in 1..N {
            if col[i] != 0 {
                dc_only = false;
                break;
            }
        }

        if dc_only && !is_dst {
            let val = (col[0] * 64 + (1 << (shift2 - 1))) >> shift2;
            let clipped = val.clamp(0, (1 << bit_depth) - 1);
            for c in 0..N {
                col_out[c] = clipped;
            }
        } else {
            transform_1d(col, &mut col_out[..N], shift2, false);
            for c in 0..N {
                col_out[c] = col_out[c].clamp(0, (1 << bit_depth) - 1);
            }
        }

        for c in 0..N {
            block[c * N + r] = col_out[c];
        }
    }
}

// ---------------------------------------------------------------------------
// Entry Points
// ---------------------------------------------------------------------------

pub fn idst_4x4_hevc(block: &mut [i32; 16], bit_depth: u8) {
    idct_2d_core::<4>(block, bit_depth, true, transform4_dst_1d_simd);
}

pub fn idct_4x4_hevc(block: &mut [i32; 16], bit_depth: u8) {
    idct_2d_core::<4>(block, bit_depth, false, transform4_1d_simd);
}

pub fn idct_8x8_hevc(block: &mut [i32; 64], bit_depth: u8) {
    idct_2d_core::<8>(block, bit_depth, false, transform8_1d_simd);
}

pub fn idct_16x16_hevc(block: &mut [i32; 256], bit_depth: u8) {
    idct_2d_core::<16>(block, bit_depth, false, transform16_1d_simd);
}

pub fn idct_32x32_hevc(block: &mut [i32; 1024], bit_depth: u8) {
    idct_2d_core::<32>(block, bit_depth, false, transform32_1d_simd);
}
