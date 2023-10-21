/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use crate::core_filters::colorspace::grayscale::scalar::{
    convert_rgb_to_grayscale_scalar, convert_rgb_to_grayscale_scalar_f32,
    convert_rgb_to_grayscale_scalar_u16
};

mod avx2;
mod scalar;
mod sse41;

pub fn rgb_to_grayscale_u16(r: &[u16], g: &[u16], b: &[u16], out: &mut [u16], max_value: u16) {
    convert_rgb_to_grayscale_scalar_u16(r, g, b, out, max_value);
}

pub fn rgb_to_grayscale_u8(r: &[u8], g: &[u8], b: &[u8], out: &mut [u8], max_value: u8) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "avx2")]
        {
            use crate::grayscale::avx2::convert_rgb_to_grayscale_u8_avx2;

            if is_x86_feature_detected!("avx2") {
                unsafe {
                    return convert_rgb_to_grayscale_u8_avx2(r, g, b, out);
                }
            }
        }

        #[cfg(feature = "sse41")]
        {
            use crate::grayscale::sse41::convert_rgb_to_grayscale_u8_sse41;

            if is_x86_feature_detected!("sse4.1") {
                unsafe {
                    return convert_rgb_to_grayscale_u8_sse41(r, g, b, out);
                }
            }
        }
    }
    convert_rgb_to_grayscale_scalar(r, g, b, out, max_value);
}

pub fn rgb_to_grayscale_f32(r: &[f32], g: &[f32], b: &[f32], out: &mut [f32], max_value: f32) {
    convert_rgb_to_grayscale_scalar_f32(r, g, b, out, max_value);
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    #[cfg(feature = "sse41")]
    #[bench]
    fn convert_rgb_to_grayscale_sse41_bench(b: &mut test::Bencher) {
        use crate::grayscale::sse41::convert_rgb_to_grayscale_u8_sse41;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let c1 = vec![0; dimensions];
        let c2 = vec![0; dimensions];
        let c3 = vec![0; dimensions];

        let mut c4 = vec![255; dimensions];
        b.iter(|| {
            unsafe {
                convert_rgb_to_grayscale_u8_sse41(&c1, &c2, &c3, &mut c4);
            };
        });
    }

    #[cfg(feature = "avx2")]
    #[bench]
    fn convert_rgb_to_grayscale_avx2_bench(b: &mut test::Bencher) {
        use crate::grayscale::avx2::convert_rgb_to_grayscale_u16_avx2;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let c1 = vec![0; dimensions];
        let c2 = vec![0; dimensions];
        let c3 = vec![0; dimensions];

        let mut c4 = vec![255; dimensions];
        b.iter(|| {
            unsafe {
                convert_rgb_to_grayscale_u16_avx2(&c1, &c2, &c3, &mut c4, 255);
            };
        });
    }

    #[bench]
    fn convert_rgb_to_grayscale_scalar_bench(b: &mut test::Bencher) {
        use crate::grayscale::scalar::convert_rgb_to_grayscale_scalar;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let c1 = vec![0_u16; dimensions];
        let c2 = vec![0_u16; dimensions];
        let c3 = vec![0_u16; dimensions];

        let mut c4 = vec![255; dimensions];
        b.iter(|| {
            convert_rgb_to_grayscale_scalar(&c1, &c2, &c3, &mut c4, 255);
        });
    }

    #[cfg(feature = "avx2")]
    #[bench]
    fn convert_rgb_to_grayscale_u16_avx_bench(b: &mut test::Bencher) {
        use crate::grayscale::avx2::convert_rgb_to_grayscale_u16_avx2;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let c1 = vec![0_u16; dimensions];
        let c2 = vec![0_u16; dimensions];
        let c3 = vec![0_u16; dimensions];

        let mut c4 = vec![255; dimensions];
        b.iter(|| unsafe {
            convert_rgb_to_grayscale_u16_avx2(&c1, &c2, &c3, &mut c4, 255);
        });
    }
}
