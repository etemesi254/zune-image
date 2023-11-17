/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Invert image pixels
//!
//! # Algorithm details
//!
//! ```text
//! max_value -> maximum value of an image depth
//!
//! pixel = max_value-pixel
//! ```
use std::ops::Sub;

use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;

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
    #[must_use]
    pub fn new() -> Invert {
        Self
    }
}

impl OperationsTrait for Invert {
    fn name(&self) -> &'static str {
        "Invert"
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth().bit_type();

        for channel in image.channels_mut(true) {
            match depth {
                BitType::U8 => invert(channel.reinterpret_as_mut::<u8>().unwrap()),
                BitType::U16 => invert(channel.reinterpret_as_mut::<u16>().unwrap()),
                BitType::F32 => invert(channel.reinterpret_as_mut::<f32>().unwrap()),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
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

///Invert a pixel
///
/// The formula for inverting a 8 bit pixel
///  is `pixel[x,y] = 255-pixel[x,y]`
pub fn invert<T>(in_image: &mut [T])
where
    T: NumOps<T> + Sub<Output = T> + Copy
{
    for pixel in in_image.iter_mut() {
        *pixel = T::max_val() - *pixel;
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use crate::invert::invert;

    #[bench]
    fn invert_u8(b: &mut test::Bencher) {
        let mut in_out = vec![0_u8; 800 * 800];

        b.iter(|| {
            invert(&mut in_out);
        });
    }

    #[bench]
    fn invert_u16(b: &mut test::Bencher) {
        let mut in_out = vec![0_u8; 800 * 800];

        b.iter(|| {
            invert(&mut in_out);
        });
    }
}
