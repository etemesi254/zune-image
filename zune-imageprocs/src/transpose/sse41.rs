#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "sse41")]
//! The algorithm
//!
//! First of all matrix transposition is an easy thing to implement
//! it's
//! ```rust
//! pub unsafe fn transpose(a:&[u8],b:&mut [u8],block_size:usize,stride:usize){
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
//! becomes tile into 8 by 8 sub-matrices, transpose in place, write out transposition
//!
//!
//!
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[allow(clippy::erasing_op, clippy::identity_op)]
#[rustfmt::skip]
unsafe fn transpose_16_by_16(
    in_matrix: &[u16], out: &mut [u16], in_stride: usize, out_stride: usize,
)
{
    // ensure the writes are safe
    assert!(in_stride * 7 + 8 <= in_matrix.len());
    assert!(out_stride * 7 + 8 <= in_matrix.len());

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

    // 16bit 8x8 transpose pass 1
    dct_interleave!(row0, row4);
    dct_interleave!(row1, row5);
    dct_interleave!(row2, row6);
    dct_interleave!(row3, row7);

    // transpose pass 2
    dct_interleave!(row0, row2);
    dct_interleave!(row1, row3);
    dct_interleave!(row4, row6);
    dct_interleave!(row5, row7);

    // transpose pass 3
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

pub unsafe fn transpose_sse41(
    in_matrix: &[u16], out_matrix: &mut [u16], width: usize, height: usize,
)
{
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

    if width < SMALL_WIDTH_THRESHOLD
    {
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

    for (i, in_width_stride) in in_matrix.chunks_exact(sin_height).enumerate()
    {
        for j in 0..width_iterations
        {
            let out_height_stride = &mut out_matrix[(j * height * 8) + (i * 8)..];

            transpose_16_by_16(&in_width_stride[(j * 8)..],
                out_height_stride,
                width,
                height,
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

    for i in rem_h..height
    {
        for j in 0..width
        {
            out_matrix[(j * height) + i] = in_matrix[(i * width) + j];
        }
    }
    for i in rem_w..width
    {
        for j in 0..height
        {
            out_matrix[(i * height) + j] = in_matrix[(j * width) + i];
        }
    }
}
