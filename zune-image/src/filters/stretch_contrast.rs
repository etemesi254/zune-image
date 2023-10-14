/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_imageprocs::stretch_contrast::stretch_contrast;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Linearly stretches the contrast in an image in place,
/// sending lower to image minimum and upper to image maximum.
#[derive(Default)]
pub struct StretchContrast {
    lower: u16,
    upper: u16
}

impl StretchContrast {
    /// Create a new stretch contrast filter
    ///
    /// # Arguments
    /// - lower: Lower minimum value for which pixels below this are clamped to the value
    /// - upper: Upper maximum value for which pixels above are clamped to the value
    pub fn new(lower: u16, upper: u16) -> StretchContrast {
        StretchContrast { lower, upper }
    }
}

impl OperationsTrait for StretchContrast {
    fn get_name(&self) -> &'static str {
        "Stretch Contrast"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.get_depth();

        for channel in image.get_channels_mut(true) {
            match depth.bit_type() {
                BitType::U8 => stretch_contrast(
                    channel.reinterpret_as_mut::<u8>()?,
                    self.lower as u8,
                    self.upper as u8,
                    u32::from(depth.max_value())
                )?,
                BitType::U16 => stretch_contrast(
                    channel.reinterpret_as_mut::<u16>()?,
                    self.lower,
                    self.upper,
                    u32::from(depth.max_value())
                )?,
                d => {
                    return Err(ImageErrors::ImageOperationNotImplemented(
                        self.get_name(),
                        d
                    ))
                }
            }
        }
        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
