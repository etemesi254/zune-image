/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Apply gamma correction onto an image
//!
//! This filter applies gamma correction on image pixels.
//!
//!
//!  # Algorithm details
//! The formula used is
//! ```text
//! max_value = maximum byte value
//! max_value_inv = 1.0/max_value
//! gamma_value =  passed gamma value
//! pixel = max_value_inv * pixel.powf(gamma_value);
//! ```
//!
//! # Implementation details
//! - For `u8` and `u16` , we use lookup tables to improve speed
//! - For `f32` naive execution is used
//!
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::errors::ImageErrors::ImageOperationNotImplemented;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::traits::NumOps;

fn build_gamma_lut<T: Default + NumOps<T> + Copy>(value: f32, max_value: u16) -> Vec<T> {
    let mut lut = vec![T::default(); usize::from(max_value) + 1];

    let max_usize = usize::from(max_value);
    let max_value = f32::from(max_value);
    let value_inv = 1.0 / max_value;
    // optimizer hint to remove bounds check, these values should be
    // powers of two, currently we support 255 and 65535
    assert!(lut.len().is_power_of_two());
    let lut_mask = lut.len() - 1;

    for x in 0..=max_usize {
        let pixel_f32 = (x as f32) * value_inv;
        let mut new_pix_val = max_value * pixel_f32.powf(value);

        if new_pix_val > max_value {
            new_pix_val = max_value;
        }

        lut[x & lut_mask] = T::from_f32(new_pix_val);
    }
    return lut;
}

/// Applies gamma correction to an image.
///
/// Gamma correction is a non-linear operation used to adjust the luminance of an image.
/// It alters the midtones of the image while keeping the absolute black (`0`) and absolute
/// white (`max_value`) constant.
///
/// # Algorithm
///
/// The formula used normalizes the pixel value, applies the power function, and
/// scales it back to the original bit depth:
/// ```text
/// normalized = pixel_value / max_value
/// new_pixel = max_value * (normalized ^ gamma)
/// ```
///
/// # Visual Effect
///
/// * **`gamma < 1.0`**: Lightens the midtones of the image (often used to decode/display linear images).
/// * **`gamma == 1.0`**: Identity; the image remains unchanged.
/// * **`gamma > 1.0`**: Darkens the midtones of the image (often used to encode images into sRGB space).
/// * Typical display gamma values are around `2.2` (or its inverse, `0.4545`).
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::gamma::Gamma;
/// use zune_image::errors::ImageErrors;
///
/// // Create a mid-gray image
/// let mut img = Image::fill(128_u8, ColorSpace::RGB, 100, 100);
///
/// // Darken the midtones by applying a gamma of 2.2
/// let gamma = Gamma::new(2.2);
/// gamma.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Default)]
pub struct Gamma {
    value: f32,
}

impl Gamma {
    /// Create a new gamma correction operation.
    ///
    /// # Arguments
    /// value: Ranges typical range is from 0.8-2.3
    #[must_use]
    pub fn new(value: f32) -> Gamma {
        Gamma { value }
    }
}

impl OperationsTrait for Gamma {
    fn name(&self) -> &'static str {
        "Gamma Correction"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let max_value = image.depth().max_value();

        let depth = image.depth();

        let gamma_u8 = if depth.bit_type() == BitType::U8 {
            Some(build_gamma_lut::<u8>(self.value, max_value))
        } else {
            None
        };
        let gamma_u16 = if depth.bit_type() == BitType::U16 {
            Some(build_gamma_lut::<u16>(self.value, max_value))
        } else {
            None
        };

        match depth.bit_type() {
            BitType::U8 => {
                image.par_process_regions::<u8, _>(true, |region| {
                    if let Some(gamma_u8) = gamma_u8.as_ref() {
                        for single_channel in &mut *region.channels {
                            gamma(single_channel, gamma_u8);
                        }
                    }
                })?;
            }
            BitType::U16 => {
                image.par_process_regions::<u16, _>(true, |region| {
                    if let Some(gamma_u16) = gamma_u16.as_ref() {
                        for single_channel in &mut *region.channels {
                            gamma(single_channel, gamma_u16);
                        }
                    }
                })?;
            }
            BitType::F32 => {
                image.par_process_regions::<f32, _>(true, |region| {
                    let max_f32 = f32::from(max_value);
                    let value_inv = 1.0 / max_f32;

                    for channel in &mut *region.channels {
                        channel.iter_mut().for_each(|x| {
                            // Normalize -> Pow -> Scale back
                            *x = max_f32 * (*x * value_inv).powf(self.value);
                        });
                    }
                })?;
            }
            _ => return Err(ImageOperationNotImplemented(self.name(), depth.bit_type())),
        }
        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Gamma
    }
}

#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::needless_range_loop,
    clippy::cast_precision_loss
)]
pub fn gamma<T>(pixels: &mut [T], lut: &[T])
where
    T: Copy + NumOps<T> + Default,
{
    // now do gamma correction
    for px in pixels {
        // T::one better optimizes than T::default, i think due to inline problems
        // so its better here
        *px = *lut.get((*px).to_usize()).unwrap_or(&T::one());
    }
}

#[cfg(test)]
mod tests {
    use zune_core::colorspace::ColorSpace;
    use zune_image::image::Image;
    use crate::FilterExt;

    #[test]
    fn test_imagr() {
        let image = Image::fill(128_u8, ColorSpace::RGB, 100, 100);

        image.gamma(2.4).unwrap();
    }
}
