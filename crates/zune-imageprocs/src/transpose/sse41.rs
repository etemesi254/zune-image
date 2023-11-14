/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "sse41")]
//! The algorithm
//!
//! First of all matrix transposition is an easy thing to implement
//! it's
//! ```rust
//! pub unsafe fn transpose_u16(a:&[u8],b:&mut [u8],block_size:usize,stride:usize){
//!    for i in 0..block_size/stride {
//!       for j in 0..stride {
//!           b[(i*stride)+j] = a[(j * stride) +i]
//!        }
//!    }
//!}
//! ```
//! But that's not as fast enough as i'd want it, it would incur a lot of cache misses
//! on writing to array b, since array b is moving stride wise(and has to be bounds checked)
//! so we can come up with a better solution
//!
//! A thing to note is that one can never rule out cache misses,
//! because either writing or reading will have to be handled stride wise
//! so the best way to do it is to write multiple elements per stride
//! that way, you can incur less cache misses per element, and overally
//! less operations
//!
//! Also fyi you can have in place transposition, it's a simple
//! std::mem::swap, but not the subject today
//!
//! So optimizing for this operation is simple, transposing using SIMD intrinsics
//!
//! The gist of it is simple, read some data into simd register, do some fancy packing and unpacking
//! and write back the data into memory
//!
//! The problem is how finicky and this issue becomes
//!
//! So reading data is easy, mapping it isn't
//!
//! A quick diagram
//!```text
//! ┌───┬──┬──┬─────────────┐
//! │   │  │  │             │
//! │ X │ 1│ 2│             │
//! ├───┼──┴──┴─────────────┤
//! │1_t│                   │
//! ├───┤                   │
//! │2_t│                   │
//! ├───┤                   │
//! │   │                   │
//! │   │                   │
//! │   │                   │
//! └───┴───────────────────┘
//! ```
//! Each contiguous item in the row becomes separated by a stride, so to reduce this
//! we do tiling as an optimization, i.e separate matrix transposition into smaller optimization
//! problems.
//! A good choice for me was 8 by 8 u8 sizes, so the gist of the algorithm
//! becomes tile into 8 by 8 sub-matrices, transpose_u16 in place, write out transposition
//!
//!
//!
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[allow(clippy::erasing_op, clippy::identity_op)]
#[rustfmt::skip]
unsafe fn transpose_8_by_8_u16(
    in_matrix: &[u16], out: &mut [u16], in_stride: usize, out_stride: usize,
)
{
    // ensure the writes are safe
    assert!(in_stride * 7 + 8 <= in_matrix.len(), "{},{}", in_stride * 7 + 8, in_matrix.len());
    assert!(out_stride * 7 + 8 <= out.len());

    let mut row0 = _mm_loadu_si128(in_matrix.get_unchecked(in_stride * 0..).as_ptr().cast());
    let mut row1 = _mm_loadu_si128(in_matrix.get_unchecked(in_stride * 1..).as_ptr().cast());
    let mut row2 = _mm_loadu_si128(in_matrix.get_unchecked(in_stride * 2..).as_ptr().cast());
    let mut row3 = _mm_loadu_si128(in_matrix.get_unchecked(in_stride * 3..).as_ptr().cast());
    let mut row4 = _mm_loadu_si128(in_matrix.get_unchecked(in_stride * 4..).as_ptr().cast());
    let mut row5 = _mm_loadu_si128(in_matrix.get_unchecked(in_stride * 5..).as_ptr().cast());
    let mut row6 = _mm_loadu_si128(in_matrix.get_unchecked(in_stride * 6..).as_ptr().cast());
    let mut row7 = _mm_loadu_si128(in_matrix.get_unchecked(in_stride * 7..).as_ptr().cast());

    // we have rows, let's make this happen.
    // Transpose operation borrowed from stb image
    // at https://github.com/nothings/stb/blob/8b5f1f37b5b75829fc72d38e7b5d4bcbf8a26d55/stb_image.h#L2608
    let mut tmp;
    macro_rules! dct_interleave {
        ($a:tt,$b:tt) => {
            tmp = $a;
            $a = _mm_unpacklo_epi16($a, $b);
            $b = _mm_unpackhi_epi16(tmp, $b)
        };
    }

    // 16bit 8x8 transpose_u16 pass 1
    dct_interleave!(row0, row4);
    dct_interleave!(row1, row5);
    dct_interleave!(row2, row6);
    dct_interleave!(row3, row7);

    // transpose_u16 pass 2
    dct_interleave!(row0, row2);
    dct_interleave!(row1, row3);
    dct_interleave!(row4, row6);
    dct_interleave!(row5, row7);

    // transpose_u16 pass 3
    dct_interleave!(row0, row1);
    dct_interleave!(row2, row3);
    dct_interleave!(row4, row5);
    dct_interleave!(row6, row7);

    _mm_storeu_si128(out.get_unchecked_mut(out_stride * 0..).as_mut_ptr().cast(), row0);
    _mm_storeu_si128(out.get_unchecked_mut(out_stride * 1..).as_mut_ptr().cast(), row1);
    _mm_storeu_si128(out.get_unchecked_mut(out_stride * 2..).as_mut_ptr().cast(), row2);
    _mm_storeu_si128(out.get_unchecked_mut(out_stride * 3..).as_mut_ptr().cast(), row3);
    _mm_storeu_si128(out.get_unchecked_mut(out_stride * 4..).as_mut_ptr().cast(), row4);
    _mm_storeu_si128(out.get_unchecked_mut(out_stride * 5..).as_mut_ptr().cast(), row5);
    _mm_storeu_si128(out.get_unchecked_mut(out_stride * 6..).as_mut_ptr().cast(), row6);
    _mm_storeu_si128(out.get_unchecked_mut(out_stride * 7..).as_mut_ptr().cast(), row7);
}

#[target_feature(enable = "sse4.1")]
unsafe fn transpose_8_by_8_u8(
    in_matrix: &[u8], out: &mut [u8], in_stride: usize, out_stride: usize
) {
    // Godbolt :https://godbolt.org/z/axoorxT8o
    // Stack overflow: https://stackoverflow.com/a/42316675

    assert!((7 * out_stride) <= out.len());

    assert!((7 * in_stride) <= in_matrix.len());

    let sv = _mm_set_epi8(15, 7, 14, 6, 13, 5, 12, 4, 11, 3, 10, 2, 9, 1, 8, 0);

    let mut pos = 0;

    // Load data from memory
    // Load 64 bites to ensure we only take 8 values
    let mn_0 = _mm_loadl_epi64(in_matrix.get_unchecked(pos..).as_ptr().cast());
    pos += in_stride;
    let mn_1 = _mm_loadl_epi64(in_matrix.get_unchecked(pos..).as_ptr().cast());
    pos += in_stride;
    let mv_0 = _mm_unpacklo_epi64(mn_0, mn_1);

    let mn_2 = _mm_loadl_epi64(in_matrix.get_unchecked(pos..).as_ptr().cast());
    pos += in_stride;
    let mn_3 = _mm_loadl_epi64(in_matrix.get_unchecked(pos..).as_ptr().cast());
    pos += in_stride;
    let mv_1 = _mm_unpacklo_epi64(mn_2, mn_3);

    let mn_4 = _mm_loadl_epi64(in_matrix.get_unchecked(pos..).as_ptr().cast());
    pos += in_stride;
    let mn_5 = _mm_loadl_epi64(in_matrix.get_unchecked(pos..).as_ptr().cast());
    pos += in_stride;
    let mv_2 = _mm_unpacklo_epi64(mn_4, mn_5);

    let mn_6 = _mm_loadl_epi64(in_matrix.get_unchecked(pos..).as_ptr().cast());
    pos += in_stride;
    let mn_7 = _mm_loadl_epi64(in_matrix.get_unchecked(pos..).as_ptr().cast());
    let mv_3 = _mm_unpacklo_epi64(mn_6, mn_7);

    let ov_0 = _mm_shuffle_epi8(mv_0, sv);
    let ov_1 = _mm_shuffle_epi8(mv_1, sv);
    let ov_2 = _mm_shuffle_epi8(mv_2, sv);
    let ov_3 = _mm_shuffle_epi8(mv_3, sv);

    let iv_0 = _mm_unpacklo_epi16(ov_0, ov_1);
    let iv_1 = _mm_unpackhi_epi16(ov_0, ov_1);
    let iv_2 = _mm_unpacklo_epi16(ov_2, ov_3);
    let iv_3 = _mm_unpackhi_epi16(ov_2, ov_3);

    let av_0 = _mm_unpacklo_epi32(iv_0, iv_2);
    let av_1 = _mm_unpackhi_epi32(iv_0, iv_2);
    let av_2 = _mm_unpacklo_epi32(iv_1, iv_3);
    let av_3 = _mm_unpackhi_epi32(iv_1, iv_3);

    // Now we have av1 having 0-16, av2 - 16-32 etc etc
    // So we want to extract and write only 8 bytes, as that is essentially a matrix
    // transpose_u16 of a 8 by 8 matrix writing to different strides.

    let sv_0 = _mm_unpackhi_epi64(av_0, _mm_setzero_si128());
    let sv_1 = _mm_unpackhi_epi64(av_1, _mm_setzero_si128());
    let sv_2 = _mm_unpackhi_epi64(av_2, _mm_setzero_si128());
    let sv_3 = _mm_unpackhi_epi64(av_3, _mm_setzero_si128());

    pos = 0;
    // Ensure writes are always in bounds
    // Needed to make the below writes unsafe

    _mm_storel_epi64(out.get_unchecked_mut(pos..).as_mut_ptr().cast(), av_0);
    pos += out_stride;

    _mm_storel_epi64(out.get_unchecked_mut(pos..).as_mut_ptr().cast(), sv_0);
    pos += out_stride;

    _mm_storel_epi64(out.get_unchecked_mut(pos..).as_mut_ptr().cast(), av_1);
    pos += out_stride;

    _mm_storel_epi64(out.get_unchecked_mut(pos..).as_mut_ptr().cast(), sv_1);
    pos += out_stride;

    _mm_storel_epi64(out.get_unchecked_mut(pos..).as_mut_ptr().cast(), av_2);
    pos += out_stride;

    _mm_storel_epi64(out.get_unchecked_mut(pos..).as_mut_ptr().cast(), sv_2);
    pos += out_stride;

    _mm_storel_epi64(out.get_unchecked_mut(pos..).as_mut_ptr().cast(), av_3);
    pos += out_stride;

    _mm_storel_epi64(out.get_unchecked_mut(pos..).as_mut_ptr().cast(), sv_3);
}

pub unsafe fn transpose_sse41_u16(
    in_matrix: &[u16], out_matrix: &mut [u16], width: usize, height: usize
) {
    const SMALL_WIDTH_THRESHOLD: usize = 8;

    let dimensions = width * height;

    assert_eq!(
        in_matrix.len(),
        dimensions,
        "In matrix dimensions do not match width and height"
    );

    assert_eq!(
        out_matrix.len(),
        dimensions,
        "Out matrix dimensions do not match width and height"
    );

    if width < SMALL_WIDTH_THRESHOLD {
        return crate::transpose::transpose_scalar(in_matrix, out_matrix, width, height);
    }

    // get how many iterations we can go per width
    //
    // ┌───────┬─────────┬────────┬───────┬──────┬──┐
    // │       │         │        │       │      │  │
    // │   8   │    8    │   8    │   8   │  8   │ l│
    // │       │         │        │       │      │  │
    // │       │         │        │       │      │  │
    // └───────┴─────────┴────────┴───────┴──────┴──┘
    //
    // We want to figure out how many times we can divide the width into
    // 8
    let width_iterations = width / 8;
    let sin_height = 8 * width;

    for (i, in_width_stride) in in_matrix.chunks_exact(sin_height).enumerate() {
        for j in 0..width_iterations {
            let out_height_stride = &mut out_matrix[(j * height * 8) + (i * 8)..];

            transpose_8_by_8_u16(
                &in_width_stride[(j * 8)..],
                out_height_stride,
                width,
                height
            );
        }
    }
    // Deal with the part that hasn't been copied
    //
    //
    //┌──────────┬─────┐
    //│          │     │
    //│          │     │
    //│  Done    │ B   │
    //│          │     │
    //│          │     │
    //├──────────┘-----│
    //│      C         │
    //└────────────────┘
    // Everything in region b and C isn't done
    let rem_w = width - (width & 7);
    let rem_h = height - (height & 7);

    for i in rem_h..height {
        for j in 0..width {
            out_matrix[(j * height) + i] = in_matrix[(i * width) + j];
        }
    }
    for i in rem_w..width {
        for j in 0..height {
            out_matrix[(i * height) + j] = in_matrix[(j * width) + i];
        }
    }
}

pub unsafe fn transpose_sse41_u8(
    in_matrix: &[u8], out_matrix: &mut [u8], width: usize, height: usize
) {
    const SMALL_WIDTH_THRESHOLD: usize = 8;

    let dimensions = width * height;

    assert_eq!(
        in_matrix.len(),
        dimensions,
        "In matrix dimensions do not match width and height"
    );

    assert_eq!(
        out_matrix.len(),
        dimensions,
        "Out matrix dimensions do not match width and height"
    );

    if width < SMALL_WIDTH_THRESHOLD {
        return crate::transpose::transpose_scalar(in_matrix, out_matrix, width, height);
    }

    // get how many iterations we can go per width
    //
    // ┌───────┬─────────┬────────┬───────┬──────┬──┐
    // │       │         │        │       │      │  │
    // │   8   │    8    │   8    │   8   │  8   │ l│
    // │       │         │        │       │      │  │
    // │       │         │        │       │      │  │
    // └───────┴─────────┴────────┴───────┴──────┴──┘
    //
    // We want to figure out how many times we can divide the width into
    // 8
    let width_iterations = width / 8;
    let sin_height = 8 * width;

    for (i, in_width_stride) in in_matrix.chunks_exact(sin_height).enumerate() {
        for j in 0..width_iterations {
            let out_height_stride = &mut out_matrix[(j * height * 8) + (i * 8)..];

            transpose_8_by_8_u8(
                &in_width_stride[(j * 8)..],
                out_height_stride,
                width,
                height
            );
        }
    }
    // Deal with the part that hasn't been copied
    //
    //
    //┌──────────┬─────┐
    //│          │     │
    //│          │     │
    //│  Done    │ B   │
    //│          │     │
    //│          │     │
    //├──────────┘-----│
    //│      C         │
    //└────────────────┘
    // Everything in region b and C isn't done
    let rem_w = width - (width & 7);
    let rem_h = height - (height & 7);

    for i in rem_h..height {
        for j in 0..width {
            out_matrix[(j * height) + i] = in_matrix[(i * width) + j];
        }
    }
    for i in rem_w..width {
        for j in 0..height {
            out_matrix[(i * height) + j] = in_matrix[(j * width) + i];
        }
    }
}

unsafe fn transpose_sse_float_4x4_inner(
    in_matrix: &[f32], out: &mut [f32], in_stride: usize, out_stride: usize
) {
    assert!((3 * out_stride) <= out.len());

    assert!((3 * in_stride) <= in_matrix.len());

    let mut row0 = _mm_loadu_ps(in_matrix.as_ptr().cast());
    let mut row1 = _mm_loadu_ps(in_matrix.get_unchecked(in_stride..).as_ptr().cast());
    let mut row2 = _mm_loadu_ps(in_matrix.get_unchecked(in_stride * 2..).as_ptr().cast());
    let mut row3 = _mm_loadu_ps(in_matrix.get_unchecked(in_stride * 3..).as_ptr().cast());

    _MM_TRANSPOSE4_PS(&mut row0, &mut row1, &mut row2, &mut row3);

    _mm_storeu_ps(out.as_mut_ptr().cast(), row0);
    _mm_storeu_ps(
        out.get_unchecked_mut(out_stride..).as_mut_ptr().cast(),
        row1
    );
    _mm_storeu_ps(
        out.get_unchecked_mut(out_stride * 2..).as_mut_ptr().cast(),
        row2
    );
    _mm_storeu_ps(
        out.get_unchecked_mut(out_stride * 3..).as_mut_ptr().cast(),
        row3
    );
}

pub unsafe fn transpose_sse_float(
    in_matrix: &[f32], out_matrix: &mut [f32], width: usize, height: usize
) {
    const SMALL_WIDTH_THRESHOLD: usize = 4;

    let dimensions = width * height;

    assert_eq!(
        in_matrix.len(),
        dimensions,
        "In matrix dimensions do not match width and height"
    );

    assert_eq!(
        out_matrix.len(),
        dimensions,
        "Out matrix dimensions do not match width and height"
    );

    if width < SMALL_WIDTH_THRESHOLD {
        return crate::transpose::transpose_scalar(in_matrix, out_matrix, width, height);
    }

    // We want to figure out how many times we can divide the width into
    // 4 since inner loop transposes by 4
    let width_iterations = width / 4;
    let sin_height = 4 * width;

    for (i, in_width_stride) in in_matrix.chunks_exact(sin_height).enumerate() {
        for j in 0..width_iterations {
            let out_height_stride = &mut out_matrix[(j * height * 4) + (i * 4)..];

            transpose_sse_float_4x4_inner(
                &in_width_stride[(j * 4)..],
                out_height_stride,
                width,
                height
            );
        }
    }
    // Deal with the part that hasn't been copied
    //
    //
    //┌──────────┬─────┐
    //│          │     │
    //│          │     │
    //│  Done    │ B   │
    //│          │     │
    //│          │     │
    //├──────────┘-----│
    //│      C         │
    //└────────────────┘
    // Everything in region b and C isn't done
    let rem_w = width - (width & 3);
    let rem_h = height - (height & 3);

    for i in rem_h..height {
        for j in 0..width {
            out_matrix[(j * height) + i] = in_matrix[(i * width) + j];
        }
    }
    for i in rem_w..width {
        for j in 0..height {
            out_matrix[(i * height) + j] = in_matrix[(j * width) + i];
        }
    }
}

pub unsafe fn transpose_sse_u32_inner(
    in_matrix: &[u32], out: &mut [u32], in_stride: usize, out_stride: usize
) {
    assert!((3 * out_stride) <= out.len());

    assert!((3 * in_stride) <= in_matrix.len());

    let mut row0 = _mm_loadu_ps(in_matrix.as_ptr().cast());
    let mut row1 = _mm_loadu_ps(in_matrix.get_unchecked(in_stride..).as_ptr().cast());
    let mut row2 = _mm_loadu_ps(in_matrix.get_unchecked(in_stride * 2..).as_ptr().cast());
    let mut row3 = _mm_loadu_ps(in_matrix.get_unchecked(in_stride * 3..).as_ptr().cast());

    _MM_TRANSPOSE4_PS(&mut row0, &mut row1, &mut row2, &mut row3);

    _mm_storeu_ps(out.as_mut_ptr().cast(), row0);
    _mm_storeu_ps(
        out.get_unchecked_mut(out_stride..).as_mut_ptr().cast(),
        row1
    );
    _mm_storeu_ps(
        out.get_unchecked_mut(out_stride * 2..).as_mut_ptr().cast(),
        row2
    );
    _mm_storeu_ps(
        out.get_unchecked_mut(out_stride * 3..).as_mut_ptr().cast(),
        row3
    );
}

pub unsafe fn transpose_sse_u32(
    in_matrix: &[u32], out_matrix: &mut [u32], width: usize, height: usize
) {
    const SMALL_WIDTH_THRESHOLD: usize = 4;

    let dimensions = width * height;

    assert_eq!(
        in_matrix.len(),
        dimensions,
        "In matrix dimensions do not match width and height"
    );

    assert_eq!(
        out_matrix.len(),
        dimensions,
        "Out matrix dimensions do not match width and height"
    );

    if width < SMALL_WIDTH_THRESHOLD {
        return crate::transpose::transpose_scalar(in_matrix, out_matrix, width, height);
    }

    // We want to figure out how many times we can divide the width into
    // 4 since inner loop transposes by 4
    let width_iterations = width / 4;
    let sin_height = 4 * width;

    for (i, in_width_stride) in in_matrix.chunks_exact(sin_height).enumerate() {
        for j in 0..width_iterations {
            let out_height_stride = &mut out_matrix[(j * height * 4) + (i * 4)..];

            transpose_sse_u32_inner(
                &in_width_stride[(j * 4)..],
                out_height_stride,
                width,
                height
            );
        }
    }
    // Deal with the part that hasn't been copied
    //
    //
    //┌──────────┬─────┐
    //│          │     │
    //│          │     │
    //│  Done    │ B   │
    //│          │     │
    //│          │     │
    //├──────────┘-----│
    //│      C         │
    //└────────────────┘
    // Everything in region b and C isn't done
    let rem_w = width - (width & 3);
    let rem_h = height - (height & 3);

    for i in rem_h..height {
        for j in 0..width {
            out_matrix[(j * height) + i] = in_matrix[(i * width) + j];
        }
    }
    for i in rem_w..width {
        for j in 0..height {
            out_matrix[(i * height) + j] = in_matrix[(j * width) + i];
        }
    }
}
