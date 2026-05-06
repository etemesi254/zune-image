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
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::traits::NumOps;
use crate::utils::{execute_on};
/// Applies a 2D convolution filter to the image.
///
/// Convolution is a fundamental spatial operation that recalculates a pixel's value
/// based on its own value and the values of its immediate neighbors. This is achieved
/// by sliding a weight matrix (also called a kernel) over the image.
///
/// This operation is the building block for many standard filters, such as blurs,
/// edge detection, sharpening, and embossing.
///
/// # Kernel Sizes
///
/// This implementation highly optimizes standard matrix sizes. The `weights` vector
/// provided **must** have a length of exactly:
/// * `9` (for a 3x3 kernel)
/// * `25` (for a 5x5 kernel)
/// * `49` (for a 7x7 kernel)
///
/// # Scaling
///
/// The `scale` parameter is multiplied by the final sum of the convolution. To maintain
/// the image's overall brightness, this is typically set to the reciprocal of the sum
/// of all weights in the matrix (e.g., `1.0 / weights.iter().sum::<f32>()`).
///
/// # Alpha Channel
///
/// The alpha channel is currently ignored by this operation.
///
/// # Example: 3x3 Edge Detection
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::errors::ImageErrors;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::convolve::Convolve;
///
/// // A standard 3x3 ridge detection (edge) matrix
/// let weights = vec![
///     -1.0, -1.0, -1.0,
///     -1.0,  8.0, -1.0,
///     -1.0, -1.0, -1.0
/// ];
/// let scale = 1.0; // Weights sum to 0, so no scaling is needed
///
/// let mut image = Image::fill(0.0f32, ColorSpace::Luma, 100, 100);
/// let convolve = Convolve::new(weights, scale);
/// convolve.execute(&mut image)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Default)]
pub struct Convolve {
    weights: Vec<f32>,
    scale: f32,
}

impl Convolve {
    /// Creates a new convolution filter.
    ///
    /// This supports 3x3, 5x5, and 7x7 matrices.
    ///
    /// # Panics
    ///
    /// This method panics if the `weights` vector length is not exactly `9` (3x3),
    /// `25` (5x5), or `49` (7x7). If you are building weights dynamically at runtime
    /// and want to handle this safely, use [`Convolve::try_new`] instead.
    #[must_use]
    pub fn new(weights: Vec<f32>, scale: f32) -> Convolve {
        Self::try_new(weights, scale)
            .expect("Convolve matrix weights length must be exactly 9 (3x3), 25 (5x5), or 49 (7x7)")
    }

    /// Safely creates a new convolution filter.
    ///
    /// This is the non-panicking alternative to `new`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Convolve)` if the weights are valid, or an `ImageErrors` if the
    /// weights length is not exactly `9` (3x3), `25` (5x5), or `49` (7x7).
    pub fn try_new(weights: Vec<f32>, scale: f32) -> Result<Convolve, ImageErrors> {
        let len = weights.len();
        if len != 9 && len != 25 && len != 49 {
            return Err(ImageErrors::GenericStr(
                "Convolve matrix weights length must be exactly 9 (3x3), 25 (5x5), or 49 (7x7)",
            ));
        }

        Ok(Convolve { weights, scale })
    }
}
impl OperationsTrait for Convolve {
    fn name(&self) -> &'static str {
        "2D convolution"
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }
    #[allow(clippy::too_many_lines)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();
        let depth = image.depth();

        let convolve_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let mut out_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

            match depth.bit_type() {
                BitType::U8 => {
                    convolve(
                        channel.reinterpret_as::<u8>()?,
                        out_channel.reinterpret_as_mut::<u8>()?,
                        width,
                        height,
                        &self.weights,
                        self.scale,
                    )?;
                }
                BitType::U16 => {
                    convolve(
                        channel.reinterpret_as::<u16>()?,
                        out_channel.reinterpret_as_mut::<u16>()?,
                        width,
                        height,
                        &self.weights,
                        self.scale,
                    )?;
                }
                BitType::F32 => {
                    convolve(
                        channel.reinterpret_as::<f32>()?,
                        out_channel.reinterpret_as_mut::<f32>()?,
                        width,
                        height,
                        &self.weights,
                        self.scale,
                    )?;
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }

            *channel = out_channel;
            Ok(())
        };
        execute_on(convolve_fn, image, true)
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

fn convolve_inner<T, const N: usize>(
    src: &[T], dst: &mut [T], width: usize, height: usize, weights: &[f32; N], scale: f32,
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>,
{
    // kernel side length and half-width (radius)
    let k = (N as f64).sqrt() as usize; // 3, 5, or 7
    let radius = k / 2; // 1, 2, or 3

    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0_f32;

            for ky in 0..k {
                let sy = (y + ky).saturating_sub(radius).min(height - 1);
                for kx in 0..k {
                    let sx = (x + kx).saturating_sub(radius).min(width - 1);
                    let weight = weights[ky * k + kx];
                    sum += f32::from(src[sy * width + sx]) * weight;
                }
            }

            dst[y * width + x] = T::from_f32(sum * scale).zclamp(T::min_val(), T::max_val());
        }
    }
}

pub fn convolve_3x3<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 9],
    scale: f32,
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>,
{
    convolve_inner::<T, 9>(in_channel, out_channel, width, height, weights, scale);
}

pub fn convolve_5x5<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 25],
    scale: f32,
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>,
{
    convolve_inner::<T, 25>(in_channel, out_channel, width, height, weights, scale);
}

pub fn convolve_7x7<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 49],
    scale: f32,
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>,
{
    convolve_inner::<T, 49>(in_channel, out_channel, width, height, weights, scale);
}

pub fn convolve<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32],
    scale: f32,
) -> Result<(), &'static str>
where
    T: NumOps<T> + Copy + Default,
    f32: std::convert::From<T>,
{
    match weights.len() {
        9 => convolve_3x3(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap(),
            scale,
        ),
        25 => convolve_5x5(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap(),
            scale,
        ),
        49 => convolve_7x7(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap(),
            scale,
        ),
        _ => return Err("Not implemented, only works for 3x3, 5x5 and 7x7 arrays"),
    }
    Ok(())
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
        nanorand::WyRand::new_seed(23131).fill(&mut data);
        convolve_7x7(&data, &mut out, width, height, &[0.0; 49], 1.);
        assert!(out.iter().all(|x| *x == 0));
    }
}
