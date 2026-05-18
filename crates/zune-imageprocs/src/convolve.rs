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
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::traits::NumOps;
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
        let depth = image.depth();
        // Pre-allocate the destination buffer
        let mut dest_image = image.clone();

        match depth.bit_type() {
            BitType::U8 => {
                image.par_process_regions_out_of_place::<u8, _>(
                    &mut dest_image,
                    true, // ignore_alpha = true
                    |region| {
                        let _ = convolve_region(region, &self.weights, self.scale);
                    },
                )?;
            }
            BitType::U16 => {
                image.par_process_regions_out_of_place::<u16, _>(
                    &mut dest_image,
                    true,
                    |region| {
                        let _ = convolve_region(region, &self.weights, self.scale);
                    },
                )?;
            }
            BitType::F32 => {
                image.par_process_regions_out_of_place::<f32, _>(
                    &mut dest_image,
                    true,
                    |region| {
                        let _ = convolve_region(region, &self.weights, self.scale);
                    },
                )?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }

        // Overwrite original image with the convolved output
        *image = dest_image;

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

fn convolve_region_inner<T, const N: usize>(
    region: &mut PlanarRegionOut<'_, T>, weights: &[f32; N], scale: f32,
) where
    T: NumOps<T> + Copy + Default + Send + Sync,
    f32: From<T>,
{
    let k = (N as f64).sqrt() as usize; // 3, 5, or 7
    let radius = k / 2; // 1, 2, or 3
    let width = region.width;

    if region.src_channels.is_empty() {
        return;
    }

    // Since src_channels contains the full image slice, we derive the global height
    let global_height = region.src_channels[0].len() / width;

    // Process each channel independently
    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        // Loop over only the chunk of rows this specific thread owns
        for local_y in 0..region.height {
            // Map to the global coordinate so we can read neighbor pixels safely
            let global_y = region.y_offset + local_y;

            for x in 0..width {
                let mut sum = 0.0_f32;

                for ky in 0..k {
                    // Safety: Clamp against the GLOBAL height
                    let sy = (global_y + ky)
                        .saturating_sub(radius)
                        .min(global_height - 1);

                    for kx in 0..k {
                        let sx = (x + kx).saturating_sub(radius).min(width - 1);
                        let weight = weights[ky * k + kx];
                        sum += f32::from(src[sy * width + sx]) * weight;
                    }
                }

                // Write out directly to the local chunk destination
                dest[local_y * width + x] =
                    T::from_f32(sum * scale).zclamp(T::min_val(), T::max_val());
            }
        }
    }
}

pub fn convolve_region<T>(
    region: &mut PlanarRegionOut<'_, T>, weights: &[f32], scale: f32,
) -> Result<(), &'static str>
where
    T: NumOps<T> + Copy + Default + Send + Sync,
    f32: From<T>,
{
    match weights.len() {
        9 => convolve_region_inner::<T, 9>(region, weights.try_into().unwrap(), scale),
        25 => convolve_region_inner::<T, 25>(region, weights.try_into().unwrap(), scale),
        49 => convolve_region_inner::<T, 49>(region, weights.try_into().unwrap(), scale),
        _ => return Err("Not implemented, only works for 3x3, 5x5 and 7x7 arrays"),
    }
    Ok(())
}
