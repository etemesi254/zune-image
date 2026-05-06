/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Sobel derivative filter
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};


use crate::traits::NumOps;
use crate::utils::{apply_gradient_3x3, execute_on};

/// Perform a sobel image derivative.
///
/// This operation calculates the gradient of the image,
/// which represents how quickly pixel values change from
/// one point to another in both the horizontal and vertical directions.
/// The magnitude and direction of the gradient can be used to detect edges in an image.
///
/// The matrix for sobel is
///
/// Gx matrix
/// ```text
///   -1, 0, 1,
///   -2, 0, 2,
///   -1, 0, 1
/// ```
/// Gy matrix
/// ```text
/// -1,-2,-1,
///  0, 0, 0,
///  1, 2, 1
/// ```
///
/// The window is a 3x3 window.
#[derive(Default, Copy, Clone)]
pub struct Sobel;

impl Sobel {
    #[must_use]
    pub fn new() -> Sobel {
        Self
    }
}

impl OperationsTrait for Sobel {
    fn name(&self) -> &'static str {
        "Sobel"
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth().bit_type();
        let (width, height) = image.dimensions();

        let sobel_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let mut out_channel = Channel::new_with_bit_type(channel.len(), depth);
            match depth {
                BitType::U8 => sobel_int::<u8>(
                    channel.reinterpret_as()?,
                    out_channel.reinterpret_as_mut()?,
                    width,
                    height,
                ),
                BitType::U16 => sobel_int::<u16>(
                    channel.reinterpret_as()?,
                    out_channel.reinterpret_as_mut()?,
                    width,
                    height,
                ),
                BitType::F32 => sobel_float::<f32>(
                    channel.reinterpret_as()?,
                    out_channel.reinterpret_as_mut()?,
                    width,
                    height,
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }
            *channel = out_channel;
            Ok(())
        };

        execute_on(sobel_fn, image, true)
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

#[rustfmt::skip]
const SOBEL_GX_I32: [i32; 9] = [
    -1, 0, 1,
    -2, 0, 2,
    -1, 0, 1,
];
#[rustfmt::skip]
const SOBEL_GY_I32: [i32; 9] = [
    -1, -2, -1,
     0,  0,  0,
     1,  2,  1,
];
#[rustfmt::skip]
const SOBEL_GX_F32: [f32; 9] = [
    -1.0, 0.0, 1.0,
    -2.0, 0.0, 2.0,
    -1.0, 0.0, 1.0,
];
#[rustfmt::skip]
const SOBEL_GY_F32: [f32; 9] = [
    -1.0, -2.0, -1.0,
     0.0,  0.0,  0.0,
     1.0,  2.0,  1.0,
];

pub fn sobel_int<T>(in_channel: &[T], out_channel: &mut [T], width: usize, height: usize)
where
    T: Default + NumOps<T> + Copy+Send+Sync,
    i32: From<T>,
{
    apply_gradient_3x3(
        in_channel,
        out_channel,
        width,
        height,
        &SOBEL_GX_I32,
        &SOBEL_GY_I32,
    );
}

pub fn sobel_float<T>(in_channel: &[T], out_channel: &mut [T], width: usize, height: usize)
where
    T: Default + NumOps<T> + Copy+Send+Sync,
    f32: From<T>,
{
    apply_gradient_3x3(
        in_channel,
        out_channel,
        width,
        height,
        &SOBEL_GX_F32,
        &SOBEL_GY_F32,
    );
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use nanorand::Rng;

    use crate::sobel::{sobel_float, sobel_int};

    #[bench]
    fn bench_sobel_integer_u8(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;

        let mut pixels = vec![0_u8; width * height];
        let mut out_pixels = vec![0; width * height];

        nanorand::WyRand::new().fill(&mut pixels);

        b.iter(|| sobel_int(&pixels, &mut out_pixels, width, height));
    }

    #[bench]
    fn bench_sobel_float(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;

        let mut pixels = vec![0.; width * height];
        let mut out_pixels = vec![0.; width * height];

        nanorand::WyRand::new().fill(&mut pixels);

        b.iter(|| sobel_float(&pixels, &mut out_pixels, width, height));
    }
}
