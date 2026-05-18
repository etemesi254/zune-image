/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Calculate image contrast
//!
//! # Algorithm
//!
//! Algorithm is from [here](https://www.dfstudios.co.uk/articles/programming/image-programming-algorithms/image-processing-algorithms-part-5-contrast-adjustment/)
//!
//! The first step is to calculate a contrast correlation factor:
//!
//! ```text
//! F = 259(C + 255) / (255(259 - C))
//! ```
//! `C` is the desired level of contrast (typically between `-255.0` and `255.0`).
//! `F` is the constant correlation factor.
//!
//! The next step is to perform the contrast adjustment around the bit-depth's midpoint.
//! For 8-bit images, the midpoint is 128:
//! ```text
//! R' = F(R - 128) + 128
//! ```

use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

/// Adjust the contrast of an image
///
/// This shifts the pixel values away from or towards their mathematical midpoint
/// (128 for 8-bit, 32768 for 16-bit, 0.5 for f32).
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::contrast::Contrast;
/// use zune_image::errors::ImageErrors;
///
/// fn main() -> Result<(), ImageErrors> {
///     let mut im = Image::fill(100_u8, ColorSpace::RGB, 100, 100);
///     let contrast = Contrast::new(10.0);
///     contrast.execute(&mut im)
/// }
/// ```
#[derive(Default)]
pub struct Contrast {
    contrast: f32,
}

impl Contrast {
    #[must_use]
    pub fn new(contrast: f32) -> Contrast {
        Contrast { contrast }
    }
}

impl OperationsTrait for Contrast {
    fn name(&self) -> &'static str {
        "contrast"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();

        // Calculate the core factor once (it is invariant of bit depth)
        let factor = (259.0 * (self.contrast + 255.0)) / (255.0 * (259.0 - self.contrast));

        match depth.bit_type() {
            BitType::U8 => {
                let lut_u8 = build_lut_u8(factor);

                image.par_process_regions::<u8, _>(true, |region| {
                    for channel in region.channels.iter_mut() {
                        contrast_u8(channel, &lut_u8);
                    }
                })?;
            }
            BitType::U16 => {
                let lut = build_lut_u16(factor);
                image.par_process_regions::<u16, _>(true, |region| {
                    for channel in region.channels.iter_mut() {
                        contrast_u16(channel, &lut);
                    }
                })?;
            }
            BitType::F32 => {
                // F32 cannot use a LUT, so we just pass the pre-computed factor
                image.par_process_regions::<f32, _>(true, |region| {
                    for channel in region.channels.iter_mut() {
                        contrast_f32(channel, factor);
                    }
                })?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }
        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::RGBA,
            ColorSpace::RGB,
            ColorSpace::LumaA,
            ColorSpace::Luma,
        ]
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Gamma
    }
}

// -----------------------------------------------------------------------
// LUT Generation
// -----------------------------------------------------------------------

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn build_lut_u8(factor: f32) -> [u8; 256] {
    let mut lut = [0_u8; 256];
    for (i, item) in lut.iter_mut().enumerate() {
        let float_pix = i as f32;
        *item = ((factor * (float_pix - 128.0)) + 128.0).clamp(0.0, 255.0).round() as u8;
    }
    lut
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn build_lut_u16(factor: f32) -> Vec<u16> {
    let mut lut = vec![0_u16; 65536];
    for (i, item) in lut.iter_mut().enumerate() {
        let float_pix = i as f32;
        // The midpoint for 16-bit integers is 32768
        *item = ((factor * (float_pix - 32768.0)) + 32768.0).clamp(0.0, 65535.0) as u16;
    }
    lut
}

// -----------------------------------------------------------------------
// Execution Functions
// -----------------------------------------------------------------------

pub fn contrast_u8(channel: &mut [u8], lut: &[u8; 256]) {
    for pix in channel {
        *pix = lut[*pix as usize];
    }
}

pub fn contrast_u16(channel: &mut [u16], lut: &[u16]) {
    for pix in channel {
        *pix = lut[*pix as usize];
    }
}

pub fn contrast_f32(channel: &mut [f32], factor: f32) {
    for pix in channel {
        // The midpoint for normalized f32 is 0.5
        *pix = ((factor * (*pix - 0.5)) + 0.5).clamp(0.0, 1.0);
    }
}
