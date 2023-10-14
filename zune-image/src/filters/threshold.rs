/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_core::log::warn;
use zune_imageprocs::threshold::threshold;
pub use zune_imageprocs::threshold::ThresholdMethod;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

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
///  See https://en.wikipedia.org/wiki/Thresholding_(image_processing)
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
    pub fn new(threshold: f32, method: ThresholdMethod) -> Threshold {
        Threshold { method, threshold }
    }
}

impl OperationsTrait for Threshold {
    fn get_name(&self) -> &'static str {
        "Threshold"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        if !image.get_colorspace().is_grayscale() {
            warn!("Threshold works well with grayscale images, results may be something you don't expect")
        }

        let depth = image.get_depth();
        for channel in image.get_channels_mut(true) {
            match depth.bit_type() {
                BitType::U16 => threshold(
                    channel.reinterpret_as_mut::<u16>()?,
                    self.threshold as u16,
                    self.method
                ),
                BitType::U8 => threshold(
                    channel.reinterpret_as_mut::<u8>()?,
                    self.threshold as u8,
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
        &[BitType::U8, BitType::U16]
    }
}
