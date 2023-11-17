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
use zune_image::traits::OperationsTrait;

use crate::pad::{pad, PadMethod};
use crate::spatial::spatial_NxN;
use crate::traits::NumOps;

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
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth().bit_type();
        let (width, height) = image.dimensions();

        #[cfg(not(feature = "threads"))]
        {
            for channel in image.get_channels_mut(true) {
                let mut out_channel = Channel::new_with_bit_type(channel.len(), depth);
                match depth {
                    BitType::U8 => sobel_int::<u8>(
                        channel.reinterpret_as()?,
                        out_channel.reinterpret_as_mut()?,
                        width,
                        height
                    ),
                    BitType::U16 => sobel_int::<u16>(
                        channel.reinterpret_as()?,
                        out_channel.reinterpret_as_mut()?,
                        width,
                        height
                    ),
                    BitType::F32 => sobel_float::<f32>(
                        channel.reinterpret_as()?,
                        out_channel.reinterpret_as_mut()?,
                        width,
                        height
                    ),
                    d => {
                        return Err(ImageErrors::ImageOperationNotImplemented(
                            self.get_name(),
                            d
                        ))
                    }
                }
                *channel = out_channel;
            }
        }
        #[cfg(feature = "threads")]
        {
            std::thread::scope(|s| {
                let mut t_results = vec![];
                for channel in image.channels_mut(true) {
                    let result = s.spawn(|| {
                        let mut out_channel = Channel::new_with_bit_type(channel.len(), depth);
                        match depth {
                            BitType::U8 => sobel_int::<u8>(
                                channel.reinterpret_as()?,
                                out_channel.reinterpret_as_mut()?,
                                width,
                                height
                            ),
                            BitType::U16 => sobel_int::<u16>(
                                channel.reinterpret_as()?,
                                out_channel.reinterpret_as_mut()?,
                                width,
                                height
                            ),
                            BitType::F32 => sobel_float::<f32>(
                                channel.reinterpret_as()?,
                                out_channel.reinterpret_as_mut()?,
                                width,
                                height
                            ),
                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.name(),
                                    d
                                ))
                            }
                        }
                        *channel = out_channel;
                        Ok(())
                    });
                    t_results.push(result);
                }
                t_results
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
/// Calculate sobel for f32 images
///
/// # Arguments
/// -  in_values: An array which is expected to contain 9 elements 
///  that represents a 3x3 window for which we are to calculate the sobel 
#[rustfmt::skip]
fn sobel_inner_f32<T>(c: &[T; 9]) -> T
    where
        T: NumOps<T> + Copy + Default,
        f32: std::convert::From<T>
{
    // matrix
    //  -1, 0, 1,
    //  -2, 0, 2,
    //  -1, 0, 1
    //
    let mut sum_a = 0.0;
    sum_a += (f32::from(c[0]) * -1.) + (f32::from(c[2]) * 1.);
    sum_a += (f32::from(c[3]) * -2.) + (f32::from(c[5]) * 2.);
    sum_a += (f32::from(c[6]) * -1.) + (f32::from(c[7]) * 1.);

    // matrix
    // -1,-2,-1,
    //  0, 0, 0,
    //  1, 2, 1
    let mut sum_b = 0.0;
    sum_b += (f32::from(c[0]) * -1.) + (f32::from(c[1]) * -2.);
    sum_b += (f32::from(c[2]) * -1.) + (f32::from(c[6]) * 1.);
    sum_b += (f32::from(c[7]) * 2.) + (f32::from(c[8]) * 1.);

    T::from_f32(((sum_a * sum_a) + (sum_b * sum_b)).sqrt())
}

/// Calculate sobel for int  images
///
/// # Arguments
/// -  in_values: An array which is expected to contain 9 elements 
///  that represents a 3x3 window for which we are to calculate the sobel values
#[allow(clippy::neg_multiply, clippy::identity_op)]
#[rustfmt::skip]
fn sobel_inner_i32<T>(c: &[T; 9]) -> T
    where
        T: NumOps<T> + Copy + Default,
        i32: std::convert::From<T>
{
    // matrix
    //  -1, 0, 1,
    //  -2, 0, 2,
    //  -1, 0, 1
    //
    let mut sum_a = 0;
    sum_a += (i32::from(c[0]) * -1) + (i32::from(c[2]) * 1);
    sum_a += (i32::from(c[3]) * -2) + (i32::from(c[5]) * 2);
    sum_a += (i32::from(c[6]) * -1) + (i32::from(c[7]) * 1);

    // matrix
    // -1,-2,-1,
    //  0, 0, 0,
    //  1, 2, 1
    let mut sum_b = 0;
    sum_b += (i32::from(c[0]) * -1) + (i32::from(c[1]) * -2);
    sum_b += (i32::from(c[2]) * -1) + (i32::from(c[6]) * 1);
    sum_b += (i32::from(c[7]) * 2) + (i32::from(c[8]) * 1);

    T::from_f64(f64::from((sum_a * sum_a) + (sum_b * sum_b)).sqrt())
}

/// Carry out the sobel filter for a float channel
///
/// Uses float operations hence may be slower than the equivalent integer functions
///
/// # Arguments.
/// - in_channel: Input channel for which contains image coefficients
/// - out_channel: Output channel for which we will fill with new sobel coefficients
/// - width: Width of input channel
/// - height: Height of input channel
pub fn sobel_float<T>(in_channel: &[T], out_channel: &mut [T], width: usize, height: usize)
where
    T: Default + NumOps<T> + Copy,
    f32: std::convert::From<T>
{
    //pad here
    let padded_input = pad(in_channel, width, height, 1, 1, PadMethod::Replicate);

    spatial_NxN::<_, _, 1, 9>(&padded_input, out_channel, width, height, sobel_inner_f32);
}

/// Carry out the sobel filter for an integer channel
///
/// Uses integer operations hence may be faster than the equivalent float functions
///
/// # Arguments.
/// - in_channel: Input channel for which contains image coefficients
/// - out_channel: Output channel for which we will fill with new sobel coefficients
/// - width: Width of input channel
/// - height: Height of input channel
pub fn sobel_int<T>(in_channel: &[T], out_channel: &mut [T], width: usize, height: usize)
where
    T: Default + NumOps<T> + Copy,
    i32: std::convert::From<T>
{
    //pad here
    let padded_input = pad(in_channel, width, height, 1, 1, PadMethod::Replicate);

    spatial_NxN::<_, _, 1, 9>(&padded_input, out_channel, width, height, sobel_inner_i32);
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
