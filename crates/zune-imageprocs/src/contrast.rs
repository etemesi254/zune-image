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
//! Steps repeated here for convenience
//!
//! First step is to calculate a contrast correlation factor
//!
//! ```text
//! f = 259(c+255)/(255(259-c))
//!```
//! `c` is the desired level of contrast.
//! `f` is the constant correlation factor.
//!
//! The next step is to perform the contrast adjustment
//! ```text
//! R' = F(R-128)+128
//! ```

use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

/// Adjust the contrast of an image
///
/// Note contrast is only currently implemented for 8 bit images.
///
/// # Example
///
/// ```
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::contrast::Contrast;
/// use zune_image::errors::ImageErrors;
///
/// fn main() -> Result<(),ImageErrors>{
///     let mut im = Image::fill(100_u8,ColorSpace::RGB,100,100);
///     let contrast = Contrast::new(10.0);
///     contrast.execute(&mut im)
/// }
///
/// ```
#[derive(Default)]
pub struct Contrast {
    contrast: f32
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

        for channel in image.channels_mut(true) {
            match depth.bit_type() {
                BitType::U8 => contrast_u8(channel.reinterpret_as_mut::<u8>()?, self.contrast),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
        }
        Ok(())
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::RGBA,
            ColorSpace::RGB,
            ColorSpace::LumaA,
            ColorSpace::Luma
        ]
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8]
    }
}

/// Calculate the contrast of an image
///
/// # Arguments
/// - channel: Input channel , modified in place
/// - contrast: The contrast to adjust the channel with
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn contrast_u8(channel: &mut [u8], contrast: f32) {
    // calculate correlation factor
    // These constants may not work for u16
    let factor = (259.0 * (contrast + 255.0)) / (255.0 * (259.0 - contrast));

    for pix in channel {
        let float_pix = f32::from(*pix);
        let new_val = ((factor * (float_pix - 128.0)) + 128.0).clamp(0.0, 255.0);
        // clamp should happen automatically??
        *pix = new_val as u8;
    }
}
