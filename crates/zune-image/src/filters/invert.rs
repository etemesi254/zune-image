/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::invert::invert;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Invert an image pixel.
///
/// The operation is similar to `T::max_val()-pixel`, where
/// `T::max_val()` is the maximum value for that bit-depth
/// (255 for [`u8`],65535 for [`u16`], 1 for [`f32`])
///
#[derive(Default)]
pub struct Invert;

impl Invert {
    /// Create a new invert operation
    pub fn new() -> Invert {
        Self::default()
    }
}

impl OperationsTrait for Invert {
    fn get_name(&self) -> &'static str {
        "Invert"
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.get_depth().bit_type();

        for channel in image.get_channels_mut(true) {
            match depth {
                BitType::U8 => invert(channel.reinterpret_as_mut::<u8>().unwrap()),
                BitType::U16 => invert(channel.reinterpret_as_mut::<u16>().unwrap()),
                BitType::F32 => invert(channel.reinterpret_as_mut::<f32>().unwrap()),
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

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::RGB,
            ColorSpace::RGBA,
            ColorSpace::LumaA,
            ColorSpace::Luma
        ]
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
