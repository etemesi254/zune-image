/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//!  Interchange row and columns in an image
//!
use std::sync::Once;

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::transpose::scalar::transpose_scalar;

pub(crate) mod scalar;
pub(crate) mod sse41;
mod tests;

static START: Once = Once::new();

/// Transpose an image
///
/// This mirrors the image along the image top left to bottom-right
/// diagonal
///
/// Done by swapping X and Y indices of the array representation
#[derive(Default)]
pub struct Transpose;

impl Transpose {
    #[must_use]
    pub fn new() -> Transpose {
        Transpose
    }
}

impl OperationsTrait for Transpose {
    fn name(&self) -> &'static str {
        "Transpose"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();
        let out_dim = width * height * image.depth().size_of();

        let depth = image.depth();

        for channel in image.channels_mut(false) {
            let mut out_channel = Channel::new_with_bit_type(out_dim, depth.bit_type());

            match depth.bit_type() {
                BitType::U8 => {
                    transpose_u8(
                        channel.reinterpret_as::<u8>()?,
                        out_channel.reinterpret_as_mut::<u8>()?,
                        width,
                        height
                    );
                }
                BitType::U16 => {
                    transpose_u16(
                        channel.reinterpret_as::<u16>()?,
                        out_channel.reinterpret_as_mut::<u16>()?,
                        width,
                        height
                    );
                }
                BitType::F32 => {
                    transpose_float(
                        channel.reinterpret_as()?,
                        out_channel.reinterpret_as_mut()?,
                        width,
                        height
                    );
                }
                d => {
                    return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d));
                }
            };
            *channel = out_channel;
        }

        image.set_dimensions(height, width);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

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
                unsafe {
                    return transpose_sse41_u16(in_matrix, out_matrix, width, height);
                }
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
                unsafe {
                    return transpose_sse41_u8(in_matrix, out_matrix, width, height);
                }
            }
        }
    }
    START.call_once(|| {
        trace!("Using scalar transpose u8 algorithm");
    });
    transpose_scalar(in_matrix, out_matrix, width, height);
}

pub fn transpose_float(in_matrix: &[f32], out_matrix: &mut [f32], width: usize, height: usize) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "sse41")]
        {
            use crate::transpose::sse41::transpose_sse_float;

            if is_x86_feature_detected!("sse4.1") {
                START.call_once(|| {
                    trace!("Using SSE4.1 transpose u8 algorithm");
                });
                unsafe {
                    return transpose_sse_float(in_matrix, out_matrix, width, height);
                }
            }
        }
    }
    START.call_once(|| {
        trace!("Using scalar transpose u8 algorithm");
    });
    transpose_scalar(in_matrix, out_matrix, width, height);
}

pub fn transpose_u32(in_matrix: &[u32], out_matrix: &mut [u32], width: usize, height: usize) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "sse41")]
        {
            use crate::transpose::sse41::transpose_sse_u32;

            if is_x86_feature_detected!("sse") {
                START.call_once(|| {
                    trace!("Using SSE4.1 transpose u8 algorithm");
                });
                unsafe {
                    return transpose_sse_u32(in_matrix, out_matrix, width, height);
                }
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

    use crate::transpose::sse41::transpose_sse_float;
    use crate::transpose::transpose_generic;

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

    #[bench]
    fn transpose_scalar_f32(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let in_vec = vec![255_f32; dimensions];
        let mut out_vec = vec![0_f32; dimensions];
        b.iter(|| transpose_generic(&in_vec, &mut out_vec, width, height));
    }

    #[bench]
    fn transpose_sse_f32(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let in_vec = vec![255.; dimensions];
        let mut out_vec = vec![0.; dimensions];
        b.iter(|| unsafe {
            transpose_sse_float(&in_vec, &mut out_vec, width, height);
        });
    }
}
