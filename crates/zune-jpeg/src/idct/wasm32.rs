/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! wasm32 simd128 IDCT
//!
//! Both kernels give the same bytes as the scalar ones, wrapping arithmetic
//! included. The module is only built with `simd128` on, which makes these
//! intrinsics safe to call, so there is no `unsafe` here.
//!
//! Neither kernel writes its coefficients, so `idct_simd128_4x4` keeps the
//! contract of `choose_idct_4x4_func`.

use core::arch::wasm32::{
    i32x4, i32x4_add, i32x4_mul, i32x4_replace_lane, i32x4_shl, i32x4_shr, i32x4_shuffle,
    i32x4_splat, i32x4_sub, u16x8_extract_lane, u16x8_min, u16x8_narrow_i32x4, u16x8_splat,
    u64x2_shuffle, v128, v128_any_true, v128_or
};

use crate::decoder::IDCTPtr;
use crate::idct::scalar::idct_int_1x1;

/// The second pass's rounding and the level shift, as `scalar::idct_int` adds them.
const SCALE_BITS: i32 = 512 + 65536 + (128 << 17);

/// The first pass's rounding.
const ROUNDING: i32 = 512;

/// The last row of a block starts this many strides from the first.
///
/// The length check goes ahead of the arithmetic, with every row stored on one
/// path: Cranelift recomputes a pure value in each branch that uses it, so a
/// store per branch would repeat the passes.
const ROWS_PAST_THE_FIRST: usize = 7;

#[inline(always)]
fn wa(a: v128, b: v128) -> v128 {
    i32x4_add(a, b)
}

#[inline(always)]
fn ws(a: v128, b: v128) -> v128 {
    i32x4_sub(a, b)
}

#[inline(always)]
fn wm(a: v128, b: i32) -> v128 {
    i32x4_mul(a, i32x4_splat(b))
}

/// One pass of `scalar::idct_int` over four columns or rows, `c[k]` the lanes'
/// coefficients `k`, its eight outputs before they are shifted down.
#[inline(always)]
fn pass([c0, c1, c2, c3, c4, c5, c6, c7]: [v128; 8], rounding: i32) -> [v128; 8] {
    let p1 = wm(wa(c2, c6), 2217);
    let t2 = wa(p1, wm(c6, -7567));
    let t3 = wa(p1, wm(c2, 3135));

    let t0 = i32x4_shl(wa(c0, c4), 12);
    let t1 = i32x4_shl(ws(c0, c4), 12);

    let rounding = i32x4_splat(rounding);
    let x0 = wa(wa(t0, t3), rounding);
    let x3 = wa(ws(t0, t3), rounding);
    let x1 = wa(wa(t1, t2), rounding);
    let x2 = wa(ws(t1, t2), rounding);

    let p3 = wa(c7, c3);
    let p4 = wa(c5, c1);
    let p1 = wa(c7, c1);
    let p2 = wa(c5, c3);
    let p5 = wm(wa(p3, p4), 4816);

    let p1 = wa(p5, wm(p1, -3685));
    let p2 = wa(p5, wm(p2, -10497));
    let p3 = wm(p3, -8034);
    let p4 = wm(p4, -1597);

    let t3 = wa(wm(c1, 6149), wa(p1, p4));
    let t2 = wa(wm(c3, 12586), wa(p2, p3));
    let t1 = wa(wm(c5, 8410), wa(p2, p4));
    let t0 = wa(wm(c7, 1223), wa(p1, p3));

    [
        wa(x0, t3),
        wa(x1, t2),
        wa(x2, t1),
        wa(x3, t0),
        ws(x3, t0),
        ws(x2, t1),
        ws(x1, t2),
        ws(x0, t3)
    ]
}

/// Four vectors' rows made their columns.
#[inline(always)]
fn transpose([a, b, c, d]: [v128; 4]) -> [v128; 4] {
    let ab_low = i32x4_shuffle::<0, 4, 1, 5>(a, b);
    let cd_low = i32x4_shuffle::<0, 4, 1, 5>(c, d);
    let ab_high = i32x4_shuffle::<2, 6, 3, 7>(a, b);
    let cd_high = i32x4_shuffle::<2, 6, 3, 7>(c, d);
    [
        u64x2_shuffle::<0, 2>(ab_low, cd_low),
        u64x2_shuffle::<1, 3>(ab_low, cd_low),
        u64x2_shuffle::<0, 2>(ab_high, cd_high),
        u64x2_shuffle::<1, 3>(ab_high, cd_high)
    ]
}

/// Two halves of a row clamped to 0..=255 and written to the head of `out`.
#[inline(always)]
fn store(low: v128, high: v128, out: &mut [i16]) {
    // Saturating to `u16` keeps what the clamp makes of every value.
    let samples = u16x8_min(u16x8_narrow_i32x4(low, high), u16x8_splat(255));
    let out = &mut out[..8];
    out[0] = u16x8_extract_lane::<0>(samples) as i16;
    out[1] = u16x8_extract_lane::<1>(samples) as i16;
    out[2] = u16x8_extract_lane::<2>(samples) as i16;
    out[3] = u16x8_extract_lane::<3>(samples) as i16;
    out[4] = u16x8_extract_lane::<4>(samples) as i16;
    out[5] = u16x8_extract_lane::<5>(samples) as i16;
    out[6] = u16x8_extract_lane::<6>(samples) as i16;
    out[7] = u16x8_extract_lane::<7>(samples) as i16;
}

/// Four rows of the first pass's output, their columns 0 to 3 in `left` and 4
/// to 7 in `right`, through the second pass and written from `out`'s row
/// `first`, a row `stride` samples on from the last.
#[inline(always)]
fn second_pass(left: [v128; 4], right: [v128; 4], out: &mut [i16], first: usize, stride: usize) {
    let [c0, c1, c2, c3] = transpose(left);
    let [c4, c5, c6, c7] = transpose(right);
    let [a0, a1, a2, a3, a4, a5, a6, a7] = pass([c0, c1, c2, c3, c4, c5, c6, c7], SCALE_BITS);
    let shifted = |v: [v128; 4]| transpose(v.map(|v| i32x4_shr(v, 17)));
    let [l0, l1, l2, l3] = shifted([a0, a1, a2, a3]);
    let [r0, r1, r2, r3] = shifted([a4, a5, a6, a7]);
    store(l0, r0, &mut out[first..]);
    store(l1, r1, &mut out[first + stride..]);
    store(l2, r2, &mut out[first + 2 * stride..]);
    store(l3, r3, &mut out[first + 3 * stride..]);
}

/// A block whose rows pass the end of `out`, as a bad image's may: `kernel`
/// into a block of its own, and the rows that fit copied.
#[cold]
fn short(kernel: IDCTPtr, in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize) {
    let mut block = [0; 64];
    kernel(in_vector, &mut block, 8);
    for (row, samples) in block.chunks_exact(8).enumerate() {
        if let Some(out) = out_vector.get_mut(row * stride..row * stride + 8) {
            out.copy_from_slice(samples);
        }
    }
}

/// Coefficients `column..column + 4` of row `row`.
#[inline(always)]
fn load(block: &[i32; 64], row: usize, column: usize) -> v128 {
    let at = row * 8 + column;
    i32x4(block[at], block[at + 1], block[at + 2], block[at + 3])
}

/// `scalar::idct_int`, four columns and four rows at a time.
pub fn idct_simd128(in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize) {
    if out_vector.len() < ROWS_PAST_THE_FIRST * stride + 8 {
        return short(idct_simd128, in_vector, out_vector, stride);
    }
    let block = &*in_vector;
    let left = core::array::from_fn::<v128, 8, _>(|row| load(block, row, 0));
    let right = core::array::from_fn::<v128, 8, _>(|row| load(block, row, 4));

    let dc_alone = i32x4_replace_lane::<0>(left[0], 0);
    let any = left[1..]
        .iter()
        .chain(&right)
        .fold(dc_alone, |any, &row| v128_or(any, row));
    if !v128_any_true(any) {
        return idct_int_1x1(in_vector, out_vector, stride);
    }

    let [l0, l1, l2, l3, l4, l5, l6, l7] = pass(left, ROUNDING).map(|v| i32x4_shr(v, 10));
    let [r0, r1, r2, r3, r4, r5, r6, r7] = pass(right, ROUNDING).map(|v| i32x4_shr(v, 10));
    second_pass([l0, l1, l2, l3], [r0, r1, r2, r3], out_vector, 0, stride);
    second_pass(
        [l4, l5, l6, l7],
        [r4, r5, r6, r7],
        out_vector,
        4 * stride,
        stride
    );
}

/// `scalar::idct4x4`: `idct_simd128` of a block whose other coefficients are
/// zero, which it reads as the constants they are.
pub fn idct_simd128_4x4(in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize) {
    if out_vector.len() < ROWS_PAST_THE_FIRST * stride + 8 {
        return short(idct_simd128_4x4, in_vector, out_vector, stride);
    }
    let block = &*in_vector;
    let zero = i32x4_splat(0);
    let rows = [
        load(block, 0, 0),
        load(block, 1, 0),
        load(block, 2, 0),
        load(block, 3, 0),
        zero,
        zero,
        zero,
        zero
    ];
    let [l0, l1, l2, l3, l4, l5, l6, l7] = pass(rows, ROUNDING).map(|v| i32x4_shr(v, 10));
    second_pass([l0, l1, l2, l3], [zero; 4], out_vector, 0, stride);
    second_pass([l4, l5, l6, l7], [zero; 4], out_vector, 4 * stride, stride);
}
