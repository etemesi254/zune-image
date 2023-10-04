/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::log::warn;
use zune_core::bit_depth::BitType;
use zune_imageprocs::threshold::threshold;
pub use zune_imageprocs::threshold::ThresholdMethod;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

pub struct Threshold {
    method:    ThresholdMethod,
    threshold: u16
}

impl Threshold {
    pub fn new(threshold: u16, method: ThresholdMethod) -> Threshold {
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
                    channel.reinterpret_as_mut::<u16>().unwrap(),
                    self.threshold,
                    self.method
                ),
                BitType::U8 => threshold(
                    channel.reinterpret_as_mut::<u8>().unwrap(),
                    self.threshold as u8,
                    self.method
                ),
                _ => todo!()
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
