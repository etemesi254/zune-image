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
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::traits::NumOps;
use crate::utils::execute_on;

/// Inverts the colors of an image.
///
/// This filter produces a photographic negative of the image by subtracting
/// each pixel's color value from the maximum possible value for its bit depth.
///
/// # Algorithm
///
/// The inversion is calculated per-pixel using:
/// ```text
/// new_pixel = MAX_VALUE - original_pixel
/// ```
/// Where `MAX_VALUE` is:
/// * `255` for 8-bit images (`u8`).
/// * `65535` for 16-bit images (`u16`).
/// * `1.0` for floating-point images (`f32`).
///
/// # Alpha Channel
///
/// The alpha channel is ignored during this operation. Inverting the alpha channel
/// would cause opaque pixels to become completely transparent, which is rarely the
/// desired behavior for a color invert.
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::invert::Invert;
/// use zune_image::errors::ImageErrors;
///
/// // Create a black image
/// let mut img = Image::fill(0_u8, ColorSpace::RGB, 100, 100);
///
/// // Invert the colors to make it completely white
/// Invert::new().execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
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
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Any
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth().bit_type();

        let invert_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            match depth {
                BitType::U8 => invert(channel.reinterpret_as_mut::<u8>()?),
                BitType::U16 => invert(channel.reinterpret_as_mut::<u16>()?),
                BitType::F32 => invert(channel.reinterpret_as_mut::<f32>()?),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
            Ok(())
        };

        execute_on(invert_fn, image, true)
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
        *pixel = T::MAX_VAL - *pixel;
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use zune_core::colorspace::ColorSpace;
    use zune_image::image::Image;
    use zune_image::traits::OperationsTrait;

    use crate::invert::Invert;

    #[test]
    fn test_multiband_invert() {
        let mut image = Image::fill(
            0_u8,
            ColorSpace::MultiBand(NonZeroU32::new(6).unwrap()),
            100,
            100
        );
        Invert::new().execute(&mut image).unwrap();
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
        let mut in_out = vec![0_u16; 800 * 800];

        b.iter(|| {
            invert(&mut in_out);
        });
    }
}
