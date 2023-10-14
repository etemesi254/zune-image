/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Adjust exposure of image

use zune_core::bit_depth::BitType;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Adjust exposure of image
pub struct Exposure {
    exposure: f32,
    black:    f32
}

impl Exposure {
    /// Create a new exposure filter
    ///
    /// # Arguments
    ///  - exposure: Set the exposure correction,
    ///     allowed range is from -3.0 to 3.0. Default should be zero
    ///
    /// - black: Set black level correction: Allowed range from -1.0 to 1.0. Default is zero
    pub fn new(exposure: f32, black: f32) -> Exposure {
        Exposure { exposure, black }
    }
}

impl OperationsTrait for Exposure {
    fn get_name(&self) -> &'static str {
        "Exposure"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let bit_type = image.get_depth().bit_type();

        for channel in image.get_channels_mut(true) {
            match bit_type {
                BitType::U8 => {
                    let raw_px = channel.reinterpret_as_mut::<u8>()?;
                    raw_px.iter_mut().for_each(|x| {
                        *x = ((f32::from(*x) - self.black) * self.exposure).clamp(0., 255.0) as _
                    })
                }
                BitType::U16 => {
                    let raw_px = channel.reinterpret_as_mut::<u16>()?;
                    raw_px.iter_mut().for_each(|x| {
                        *x = ((f32::from(*x) - self.black) * self.exposure).clamp(0., 65535.0) as _
                    })
                }
                BitType::F32 => {
                    let raw_px = channel.reinterpret_as_mut::<f32>()?;
                    raw_px
                        .iter_mut()
                        .for_each(|x| *x = (*x - self.black) * self.exposure)
                }
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
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
