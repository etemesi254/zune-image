/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Threshold filter: Binarize an image
use zune_core::bit_depth::BitType;
use zune_core::log::warn;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;

#[derive(Copy, Clone, Debug)]
pub enum ThresholdMethod {
    Binary,
    BinaryInv,
    ThreshTrunc,
    ThreshToZero
}

impl ThresholdMethod {
    pub fn from_string_result(input: &str) -> Result<Self, String> {
        match input
        {
            "binary" => Ok(Self::Binary),
            "binary_inv" => Ok(Self::BinaryInv),
            "thresh_trunc" => Ok(Self::ThreshTrunc),
            "thresh_to_zero" => Ok(Self::ThreshToZero),
            _ => Err("Unknown threshold type,accepted values are binary,binary_inv,thresh_trunc,thresh_to_zero".to_string()),
        }
    }
}

/// Apply a fixed level threshold to an image.
///
///
/// # Methods
/// The library supports threshold methods derived from opencv , see [here](https://docs.opencv.org/4.x/d7/d1b/group__imgproc__misc.html#gaa9e58d2860d4afa658ef70a9b1115576)
/// for the definitions
///
///  - [Binary](ThresholdMethod::Binary) => max if src(x,y) > thresh 0 otherwise
///  - [BinaryInv](ThresholdMethod::BinaryInv) => 0 if src(x,y) > thresh max otherwise
///  - [ThreshTrunc](ThresholdMethod::ThreshTrunc) => thresh if src(x,y) > thresh src(x,y) otherwise
///  - [ThreshToZero](ThresholdMethod::ThreshToZero) => src(x,y) if src(x,y) > thresh 0 otherwise
///           
///  See [Wikipedia Article on Thresholding](https://en.wikipedia.org/wiki/Thresholding_(image_processing))
pub struct Threshold {
    method:    ThresholdMethod,
    threshold: f32
}

impl Threshold {
    /// Create a new threshold filter
    ///
    /// # Arguments
    /// - threshold: f32 The maximum value, this is type casted to the appropriate bit depth
    /// for 8 bit images it saturates at u8::MAX, for 16 bit images at u16::MAX, for float images
    /// the value is treated as is
    /// - method: Threshold method to use, matches opencv methods
    #[must_use]
    pub fn new(threshold: f32, method: ThresholdMethod) -> Threshold {
        Threshold { method, threshold }
    }
}

impl OperationsTrait for Threshold {
    fn name(&self) -> &'static str {
        "Threshold"
    }

    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation
    )]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        if !image.colorspace().is_grayscale() {
            warn!("Threshold works well with grayscale images, results may be something you don't expect");
        }

        let depth = image.depth();
        for channel in image.channels_mut(true) {
            match depth.bit_type() {
                BitType::U16 => threshold(
                    channel.reinterpret_as_mut::<u16>()?,
                    self.threshold.clamp(0., 65535.) as u16,
                    self.method
                ),
                BitType::U8 => threshold(
                    channel.reinterpret_as_mut::<u8>()?,
                    self.threshold.clamp(0., 255.) as u8,
                    self.method
                ),
                BitType::F32 => threshold(
                    channel.reinterpret_as_mut::<f32>()?,
                    self.threshold,
                    self.method
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented("threshold", d))
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
#[rustfmt::skip]
pub fn threshold<T>(in_channel: &mut [T], threshold: T, method: ThresholdMethod)
    where
        T: NumOps<T> + Copy + PartialOrd
{
    let max = T::max_val();
    let min = T::min_val();
    match method
    {
        ThresholdMethod::Binary =>
            {
                for x in in_channel.iter_mut()
                {
                    *x = if *x > threshold { max } else { min };
                }
            }
        ThresholdMethod::BinaryInv =>
            {
                for x in in_channel.iter_mut()
                {
                    *x = if *x > threshold { min } else { max };
                }
            }
        ThresholdMethod::ThreshTrunc =>
            {
                for x in in_channel.iter_mut()
                {
                    *x = if *x > threshold { threshold } else { *x };
                }
            }
        ThresholdMethod::ThreshToZero =>
            {
                for x in in_channel.iter_mut()
                {
                    *x = if *x > threshold { threshold } else { T::min_val() }
                }
            }
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    #[bench]
    fn threshold_scalar_u8(b: &mut test::Bencher) {
        use crate::threshold::threshold;

        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0_u8; dimensions];

        b.iter(|| {
            threshold(&mut c1, 10, crate::threshold::ThresholdMethod::BinaryInv);
        });
    }

    #[bench]
    fn threshold_scalar_u16(b: &mut test::Bencher) {
        use crate::threshold::threshold;

        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0_u16; dimensions];

        b.iter(|| {
            threshold(&mut c1, 10, crate::threshold::ThresholdMethod::BinaryInv);
        });
    }
}
