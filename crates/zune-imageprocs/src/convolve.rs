/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! 2D convolution on images
//!
//! This filter adds support for common image convolving
//! for 3x3, 5x5 and 7x7 convolutions.
//!
//! The intermediate calculations are carried in `f32`
//!

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::pad::{pad, PadMethod};
use crate::traits::NumOps;
use crate::utils::z_prefetch;

/// Convolve an image
///
///
///  # Alpha channel
/// - Alpha channel is ignored
///
/// # Example
/// - Convolve with a 3x3 filter matrix
///
/// ```
/// // Create a 3x3 matrix
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::errors::ImageErrors;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::convolve::Convolve;
/// let matrix = vec![1.0, -1.0,  1.0,
///                  -1.0,  1.0, -1.0,
///                   1.0, -1.0,  1.0];
/// // scale is  multiplied by the result of the convolution, let's use
/// // it's reciprocal
/// let scale = 1.0/matrix.iter().sum::<f32>();
///
/// let inv_scale = 1.0 / (100*100) as f32;
/// // create a luma image that starts from black and ends as white
/// let mut  image = Image::from_fn::<f32,_>(100,100,ColorSpace::Luma,|x,y,pix|{
///     pix[0] = ((x+y) as f32) * inv_scale ;
/// });
/// // convolve finally
/// let new_image = Convolve::new(matrix,scale).execute(&mut image)?;
/// # Ok::<(),ImageErrors>(())
/// ```
#[derive(Default)]
pub struct Convolve {
    weights: Vec<f32>,
    scale:   f32
}

impl Convolve {
    /// Create a new convolve matrix, this supports 3x3,5x5 and 7x7 matrices
    ///
    /// The operation will return an error if the weights length isn't 9(3x3),25(5x5) or 49(7x7)
    #[must_use]
    pub fn new(weights: Vec<f32>, scale: f32) -> Convolve {
        Convolve { weights, scale }
    }
}

impl OperationsTrait for Convolve {
    fn name(&self) -> &'static str {
        "2D convolution"
    }
    #[allow(clippy::too_many_lines)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();
        let depth = image.depth();

        #[cfg(feature = "threads")]
        {
            trace!("Running convolve in multithreaded mode");

            std::thread::scope(|s| {
                let mut errors = vec![];
                for channel in image.channels_mut(true) {
                    let scope = s.spawn(|| {
                        // Hello
                        let mut out_channel = Channel::new_with_bit_type(
                            width * height * depth.size_of(),
                            depth.bit_type()
                        );

                        match depth.bit_type() {
                            BitType::U8 => {
                                convolve(
                                    channel.reinterpret_as::<u8>()?,
                                    out_channel.reinterpret_as_mut::<u8>()?,
                                    width,
                                    height,
                                    &self.weights,
                                    self.scale
                                )?;
                            }
                            BitType::U16 => {
                                convolve(
                                    channel.reinterpret_as::<u16>()?,
                                    out_channel.reinterpret_as_mut::<u16>()?,
                                    width,
                                    height,
                                    &self.weights,
                                    self.scale
                                )?;
                            }
                            BitType::F32 => {
                                convolve(
                                    channel.reinterpret_as::<f32>()?,
                                    out_channel.reinterpret_as_mut::<f32>()?,
                                    width,
                                    height,
                                    &self.weights,
                                    self.scale
                                )?;
                            }
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
                    errors.push(scope);
                }
                errors
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }
        #[cfg(not(feature = "threads"))]
        {
            for channel in image.get_channels_mut(true) {
                let mut out_channel =
                    Channel::new_with_bit_type(width * height * depth.size_of(), depth.bit_type());

                match depth.bit_type() {
                    BitType::U8 => {
                        convolve(
                            channel.reinterpret_as::<u8>()?,
                            out_channel.reinterpret_as_mut::<u8>()?,
                            width,
                            height,
                            &self.weights,
                            self.scale
                        )?;
                    }
                    BitType::U16 => {
                        convolve(
                            channel.reinterpret_as::<u16>()?,
                            out_channel.reinterpret_as_mut::<u16>()?,
                            width,
                            height,
                            &self.weights,
                            self.scale
                        )?;
                    }
                    BitType::F32 => {
                        convolve(
                            channel.reinterpret_as::<f32>()?,
                            out_channel.reinterpret_as_mut::<f32>()?,
                            width,
                            height,
                            &self.weights,
                            self.scale
                        )?;
                    }
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
        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

fn convolve_3x3_inner<T>(in_array: &[T; 9], weights: &[f32; 9], scale: f32) -> T
where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    T::from_f32(
        in_array
            .iter()
            .zip(weights)
            .map(|(x, weight)| f32::from(*x) * weight)
            .sum::<f32>()
            * scale
    )
    .zclamp(T::min_val(), T::max_val())
}

fn convolve_5x5_inner<T>(in_array: &[T; 25], weights: &[f32; 25], scale: f32) -> T
where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    T::from_f32(
        in_array
            .iter()
            .zip(weights)
            .map(|(x, weight)| f32::from(*x) * weight)
            .sum::<f32>()
            * scale
    )
    .zclamp(T::min_val(), T::max_val())
}

fn convolve_7x7_inner<T>(in_array: &[T; 49], weights: &[f32; 49], scale: f32) -> T
where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    T::from_f32(
        in_array
            .iter()
            .zip(weights)
            .map(|(x, weight)| f32::from(*x) * weight)
            .sum::<f32>()
            * scale
    )
    .zclamp(T::min_val(), T::max_val())
}

/// Convolve a matrix
pub fn convolve_3x3<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 9],
    scale: f32
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    // pad input
    //pad here
    let padded_input = pad(in_channel, width, height, 1, 1, PadMethod::Replicate);

    spatial_NxN::<T, _, 1, 9>(
        &padded_input,
        out_channel,
        width,
        height,
        convolve_3x3_inner,
        weights,
        scale
    );
}

pub fn convolve_5x5<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 25],
    scale: f32
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    // pad input
    //pad here
    let padded_input = pad(in_channel, width, height, 2, 2, PadMethod::Replicate);

    spatial_NxN::<T, _, 2, 25>(
        &padded_input,
        out_channel,
        width,
        height,
        convolve_5x5_inner,
        weights,
        scale
    );
}

pub fn convolve_7x7<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 49],
    scale: f32
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    // pad input
    //pad here
    let padded_input = pad(in_channel, width, height, 3, 3, PadMethod::Replicate);

    spatial_NxN::<T, _, 3, 49>(
        &padded_input,
        out_channel,
        width,
        height,
        convolve_7x7_inner,
        weights,
        scale
    );
}

/// Selects a convolve matrix
pub fn convolve<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32],
    scale: f32
) -> Result<(), &'static str>
where
    T: NumOps<T> + Copy + Default,
    f32: std::convert::From<T>
{
    if weights.len() == 9 {
        convolve_3x3::<T>(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap(),
            scale
        );
    } else if weights.len() == 25 {
        convolve_5x5::<T>(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap(),
            scale
        );
    } else if weights.len() == 49 {
        convolve_7x7::<T>(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap(),
            scale
        );
    } else {
        return Err("Not implemented, only works for 3x3, 5x5 and 7x7 arrays");
    }
    Ok(())
}

/// A special spatial function that takes advantage of const generics to
/// speed up operations for convolve
#[allow(non_snake_case)]
fn spatial_NxN<T, F, const RADIUS: usize, const OUT_SIZE: usize>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, function: F,
    values: &[f32; OUT_SIZE], scale: f32
) where
    T: Default + Copy,
    F: Fn(&[T; OUT_SIZE], &[f32; OUT_SIZE], f32) -> T
{
    let old_width = width;
    let height = (RADIUS * 2) + height;
    let width = (RADIUS * 2) + width;

    assert_eq!(height * width, in_channel.len());

    let radius_size = (2 * RADIUS) + 1;

    let radius_loop = radius_size >> 1;

    let mut local_storage = [T::default(); OUT_SIZE];

    for y in radius_loop..height - radius_loop {
        for x in radius_loop..width - radius_loop {
            let iy = y - radius_loop;
            let ix = x - radius_loop;

            let mut i = 0;

            for ky in 0..radius_size {
                let iy_i = iy + ky;

                let in_slice = &in_channel[(iy_i * width) + ix..(iy_i * width) + ix + radius_size];
                z_prefetch(in_channel, (iy_i + 1) * width + ix);
                local_storage[i..i + radius_size].copy_from_slice(in_slice);
                z_prefetch(in_channel, (iy_i + 2) * width + ix);

                i += radius_size;
            }

            let result = function(&local_storage, values, scale);

            out_channel[iy * old_width + ix] = result;
        }
    }
}

#[cfg(test)]
mod tests {
    use nanorand::Rng;

    use crate::convolve::{convolve_3x3, convolve_5x5, convolve_7x7};

    // test that 3x3 convolution works
    #[test]
    fn convolve_3x3_test() {
        let (width, height) = (100, 100);
        let mut data = vec![0u8; width * height];
        let mut out = vec![13; width * height];
        nanorand::WyRand::new().fill(&mut data);
        convolve_3x3(&data, &mut out, width, height, &[0.0; 9], 1.);
        assert!(out.iter().all(|x| *x == 0));
    }

    #[test]
    fn convolve_5x5_test() {
        let (width, height) = (100, 100);
        let mut data = vec![0u8; width * height];
        let mut out = vec![13; width * height];
        nanorand::WyRand::new().fill(&mut data);
        convolve_5x5(&data, &mut out, width, height, &[0.0; 25], 1.);
        assert!(out.iter().all(|x| *x == 0));
    }

    #[test]
    fn convolve_7x7_test() {
        let (width, height) = (100, 100);
        let mut data = vec![0u8; width * height];
        let mut out = vec![13; width * height];
        nanorand::WyRand::new().fill(&mut data);
        convolve_7x7(&data, &mut out, width, height, &[0.0; 49], 1.);
        assert!(out.iter().all(|x| *x == 0));
    }
}
