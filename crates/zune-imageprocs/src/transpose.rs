/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::sync::Once;

use log::trace;

use crate::transpose::scalar::transpose_scalar;

pub(crate) mod scalar;
pub(crate) mod sse41;
mod tests;

static START: Once = Once::new();

pub fn transpose_u16(in_matrix: &[u16], out_matrix: &mut [u16], width: usize, height: usize) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "sse41")]
        {
            use crate::transpose::sse41::transpose_sse41_u16;

            if is_x86_feature_detected!("sse4.1") {
                START.call_once(|| {
                    trace!("Using SSE4.1 transpose_u16 algorithm");
                });
                unsafe { return transpose_sse41_u16(in_matrix, out_matrix, width, height) }
            }
        }
    }
    START.call_once(|| {
        trace!("Using scalar transpose_u16 algorithm");
    });
    transpose_scalar(in_matrix, out_matrix, width, height);
}

pub fn transpose_u8(in_matrix: &[u8], out_matrix: &mut [u8], width: usize, height: usize) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "sse41")]
        {
            use crate::transpose::sse41::transpose_sse41_u8;

            if is_x86_feature_detected!("sse4.1") {
                START.call_once(|| {
                    trace!("Using SSE4.1 transpose u8 algorithm");
                });
                unsafe { return transpose_sse41_u8(in_matrix, out_matrix, width, height) }
            }
        }
    }
    START.call_once(|| {
        trace!("Using scalar transpose u8 algorithm");
    });
    transpose_scalar(in_matrix, out_matrix, width, height);
}

pub fn transpose_generic<T: Default + Copy>(
    in_matrix: &[T], out_matrix: &mut [T], width: usize, height: usize
) {
    transpose_scalar(in_matrix, out_matrix, width, height);
}

#[cfg(all(feature = "benchmarks", feature = "sse41"))]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    #[bench]
    fn transpose_sse_u16(b: &mut test::Bencher) {
        use crate::transpose::sse41::transpose_sse41_u16;
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let in_vec = vec![255; dimensions];
        let mut out_vec = vec![0; dimensions];
        b.iter(|| {
            unsafe {
                transpose_sse41_u16(&in_vec, &mut out_vec, width, height);
            };
        });
    }
    #[bench]
    fn transpose_scalar(b: &mut test::Bencher) {
        use crate::transpose::scalar::transpose_scalar;
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let in_vec = vec![255; dimensions];
        let mut out_vec = vec![0_u16; dimensions];
        b.iter(|| {
            transpose_scalar(&in_vec, &mut out_vec, width, height);
        });
    }

    #[bench]
    fn transpose_sse_u8(b: &mut test::Bencher) {
        use crate::transpose::sse41::transpose_sse41_u8;
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let in_vec = vec![255; dimensions];
        let mut out_vec = vec![0_u8; dimensions];
        b.iter(|| unsafe {
            transpose_sse41_u8(&in_vec, &mut out_vec, width, height);
        });
    }
}
