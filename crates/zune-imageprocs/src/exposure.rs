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
use zune_image::traits::OperationsTrait;

/// Adjust exposure of image
///
/// Read [module-docs](crate::exposure) for algorithm details and gotchas
///
/// # Alpha channel
/// - Alpha channel is ignored
///
/// # Example
/// ```
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::errors::ImageErrors;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::exposure::Exposure;
///
/// // create a 100x100 grayscale image
/// let mut img = Image::from_fn::<u16,_>(100,100,ColorSpace::Luma,|x,y,pix|{
///    pix[0]=((x + y) % 65536) as u16;
/// });
/// // increase each pixels strength by 2
/// Exposure::new(2.0,0.0).execute(&mut img)?;
///
///# Ok::<(),ImageErrors>(())
/// ```
///
pub struct Exposure {
    exposure: f32,
    black:    f32
}

impl Exposure {
    /// Create a new exposure filter
    ///
    /// # Arguments
    ///  - exposure: Set the exposure correction,
    ///     Common range is from -3.0 to 3.0. Default should be zero
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

        for channel in image.channels_mut(true) {
            match bit_type {
                BitType::U8 => {
                    let raw_px = channel.reinterpret_as_mut::<u8>()?;
                    for x in raw_px.iter_mut() {
                        *x = ((f32::from(*x) - black) * self.exposure).clamp(0., 255.0) as _;
                    }
                }
                BitType::U16 => {
                    let raw_px = channel.reinterpret_as_mut::<u16>()?;
                    for x in raw_px.iter_mut() {
                        *x = ((f32::from(*x) - black) * self.exposure).clamp(0., 65535.0) as _;
                    }
                }
                BitType::F32 => {
                    let raw_px = channel.reinterpret_as_mut::<f32>()?;
                    raw_px
                        .iter_mut()
                        .for_each(|x| *x = (*x - black) * self.exposure);
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
