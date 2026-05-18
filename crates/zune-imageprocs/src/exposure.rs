/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Exposure filter
//!
//!
//! # Formula
//!
//! The formula used to calculate exposure is
//! ```text
//! pix = clamp((pix - black) * exposure)
//! ```
//!
//! # Gotchas
//! -`f32` depth doesn't do any clamping, hence values may get out of range
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};


/// Adjusts the exposure and black level of an image.
///
/// This filter linearly scales the brightness of the image while allowing a specific
/// black-level cutoff.
///
/// # Algorithm
///
/// The adjustment is applied per-pixel using the following formula:
/// ```text
/// new_pix = clamp((old_pix - scaled_black) * exposure)
/// ```
///
/// # Gotchas
///
/// * The `f32` bit-depth implementation does not perform any clamping. Values may
///   exceed `1.0` or drop below `0.0` depending on the multipliers.
/// * The alpha channel is ignored during this operation.
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::errors::ImageErrors;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::exposure::Exposure;
///
/// // Create a 100x100 grayscale image
/// let mut img = Image::fill(100_u16, ColorSpace::Luma, 100, 100);
///
/// // Increase each pixel's brightness by a factor of 2.0, with a 0.0 black level offset.
/// Exposure::new(2.0, 0.0).execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
pub struct Exposure {
    exposure: f32,
    black: f32,
}

impl Exposure {
    /// Create a new exposure filter
    ///
    /// # Arguments
    ///  - exposure: Set the exposure correction,
    ///    Common range is from -3.0 to 3.0. Default should be 1.0
    ///
    /// - black: Set black level correction,should be between 0.0 and 1.0,
    /// it will be scaled appropriately depending on image depth. E.g
    /// for a depth of 8 it will be multiplied by 255, and for a depth of 16, by
    /// 65535
    ///
    #[must_use]
    pub fn new(exposure: f32, black: f32) -> Exposure {
        Exposure { exposure, black }
    }
}

impl OperationsTrait for Exposure {
    fn name(&self) -> &'static str {
        "Exposure"
    }

    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_lossless,
        clippy::cast_possible_truncation
    )]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let bit_type = image.depth().bit_type();
        let black = self.black.clamp(0.0, 1.0) * f32::from(image.depth().max_value());
        let exposure = self.exposure;

        // Build LUTs outside the closure so they are only calculated once
        // per execute_impl call, rather than per channel/thread.

        let mut lut_u16 = vec![0_u16; if bit_type == BitType::U16 { 65536 } else { 0 }];
        if bit_type == BitType::U16 {
            for (i, item) in lut_u16.iter_mut().enumerate() {
                *item = ((i as f32 - black) * exposure).clamp(0.0, 65535.0) as u16;
            }
        }

        match bit_type {
            BitType::U8 => {
                let mut lut_u8 = [0_u8; 256];
                for (i, item) in lut_u8.iter_mut().enumerate() {
                    *item = ((i as f32 - black) * exposure).clamp(0.0, 255.0).round() as u8;
                }

                image.par_process_regions::<u8, _>(true, |region| {
                    for channel in region.channels.iter_mut() {
                        for pix in channel.iter_mut() {
                            *pix = lut_u8[usize::from(*pix)];
                        }
                    }
                })?;
            }
            BitType::U16 => {
                let mut lut_u16 = vec![0_u16; 65536];
                for (i, item) in lut_u16.iter_mut().enumerate() {
                    *item = ((i as f32 - black) * exposure).clamp(0.0, 65535.0) as u16;
                }
                let lut_u16_bound: &[u16; 65536] = lut_u16.as_slice().try_into().unwrap();

                image.par_process_regions::<u16, _>(true, |region| {
                    for channel in region.channels.iter_mut() {
                        for pix in channel.iter_mut() {
                            *pix = lut_u16_bound[usize::from(*pix)];
                        }
                    }
                })?;
            }
            BitType::F32 => {
                image.par_process_regions::<f32, _>(true, |region| {
                    for channel in region.channels.iter_mut() {
                        for pix in channel.iter_mut() {
                            *pix = (*pix - black) * exposure;
                        }
                    }
                })?;
            }
            _ => {
                return Err(ImageErrors::ImageOperationNotImplemented(
                    self.name(),
                    bit_type,
                ))
            }
        }
        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }
}
