/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionMut;
use zune_image::traits::{OperationColorValues, OperationsTrait};

/// Linearly stretches the contrast of an image.
///
/// This filter remaps the pixel values of an image so that a specified `lower` bound
/// becomes the absolute minimum (e.g., `0`), and an `upper` bound becomes the absolute
/// maximum (e.g., `255`, `65535`, or `1.0`).
///
/// This is highly effective for normalizing images that suffer from poor lighting or
/// atmospheric haze, stretching their limited color range across the entire available
/// spectrum.
///
/// # Algorithm
///
/// For each pixel:
/// * If `pixel <= lower`, it is clamped to the minimum value.
/// * If `pixel >= upper`, it is clamped to the maximum value.
/// * Otherwise, it is scaled linearly: `new_pixel = (pixel - lower) / (upper - lower) * MAX`
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::stretch_contrast::StretchContrast;
/// use zune_image::errors::ImageErrors;
///
/// // Create an image
/// let mut img = Image::fill(128_u8, ColorSpace::RGB, 100, 100);
///
/// // Stretch the contrast so that any pixel <= 50 becomes 0,
/// // and any pixel >= 200 becomes 255.
/// let stretch = StretchContrast::new(50.0, 200.0);
/// stretch.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Default)]
pub struct StretchContrast {
    lower: f32,
    upper: f32,
}

impl StretchContrast {
    /// Create a new stretch contrast filter
    ///
    /// # Arguments
    /// - lower: Lower minimum value for which pixels below this are clamped to the value
    /// - upper: Upper maximum value for which pixels above are clamped to the value
    #[must_use]
    pub fn new(lower: f32, upper: f32) -> StretchContrast {
        StretchContrast { lower, upper }
    }
}

impl OperationsTrait for StretchContrast {
    fn name(&self) -> &'static str {
        "Stretch Contrast"
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        if self.upper < self.lower {
            return Err(ImageErrors::GenericStr("upper must be strictly greater than lower"));
        }

        let depth = image.depth();
        let ignore_alpha = true; // Contrast usually ignores the alpha channel

        match depth.bit_type() {
            BitType::U8 => {
                // 1. Build the LUT once (256 elements)
                let lut = build_lut_u8(self.lower as u8, self.upper as u8);

                image.par_process_regions::<u8, _>(ignore_alpha, |region| {
                    apply_lut_region(region, &lut);
                })?;
            }
            BitType::U16 => {
                // 1. Build the LUT once (65536 elements)
                let lut = build_lut_u16(self.lower as u16, self.upper as u16);

                image.par_process_regions::<u16, _>(ignore_alpha, |region| {
                    apply_lut_region(region, &lut);
                })?;
            }
            BitType::F32 => {
                // F32 has millions of possible values, so we process it mathematically
                let lower = self.lower;
                let upper = self.upper;

                image.par_process_regions::<f32, _>(ignore_alpha, |region| {
                    stretch_contrast_region_f32(region, lower, upper);
                })?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
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

fn build_lut_u8(lower: u8, upper: u8) -> [u8; 256] {
    let mut lut = [0u8; 256];
    let maximum = 255u32;
    let len = u32::from(upper.saturating_sub(lower)).saturating_add(1);

    for i in 0..=255u8 {
        if i >= upper {
            lut[i as usize] = 255;
        } else if i <= lower {
            lut[i as usize] = 0;
        } else {
            let numerator = maximum * u32::from(i - lower);
            lut[i as usize] = (numerator / len) as u8;
        }
    }
    lut
}

fn build_lut_u16(lower: u16, upper: u16) -> Vec<u16> {
    // 65536 elements take ~131 KB. We use a Vec to avoid blowing up the stack.
    let mut lut = vec![0u16; 65536];
    let maximum = 65535u32;
    let len = u32::from(upper.saturating_sub(lower)).saturating_add(1);

    for i in 0..=65535u32 {
        let val = i as u16;
        if val >= upper {
            lut[i as usize] = 65535;
        } else if val <= lower {
            lut[i as usize] = 0;
        } else {
            let numerator = maximum * u32::from(val - lower);
            lut[i as usize] = (numerator / len) as u16;
        }
    }
    lut
}
fn apply_lut_region<T>(region: &mut PlanarRegionMut<'_, T>, lut: &[T])
where
    T: Copy + Default + Send + Sync,
    usize: From<T>, // Allows us to use the pixel value as an array index safely
{
    for channel in region.channels.iter_mut() {
        for px in channel.iter_mut() {
            *px = lut[usize::from(*px)];
        }
    }
}

/// Computes the mathematical stretch directly on the f32 values
fn stretch_contrast_region_f32(region: &mut PlanarRegionMut<'_, f32>, lower: f32, upper: f32) {
    let inv_range = 1.0 / (upper - lower);

    for channel in region.channels.iter_mut() {
        for pixel in channel.iter_mut() {
            if *pixel > upper {
                *pixel = f32::MAX; // Or use your trait's f32::max_val() if preferred
            } else if *pixel <= lower {
                *pixel = f32::MIN; // Or use f32::min_val() / 0.0 depending on your spec
            } else {
                *pixel = (*pixel - lower) * inv_range;
            }
        }
    }
}