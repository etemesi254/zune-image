/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_imageprocs::gamma::gamma;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Gamma adjust an image
///
/// This currently only supports 8 and 16 bit depth images since it applies an optimization
/// that works for those depths.
///
/// This operation is internally multithreaded, where supported
#[derive(Default)]
pub struct Gamma {
    value: f32
}

impl Gamma {
    /// Create a new gamma correction operation.
    ///
    /// # Arguments
    /// value: Ranges typical range is from 0.8-2.3
    pub fn new(value: f32) -> Gamma {
        Gamma { value }
    }
}

impl OperationsTrait for Gamma {
    fn get_name(&self) -> &'static str {
        "Gamma Correction"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let max_value = image.get_depth().max_value();

        let depth = image.get_depth();
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running gamma correction in single threaded mode");

            for channel in image.get_channels_mut(false) {
                match depth.bit_type() {
                    BitType::U16 => {
                        gamma(channel.reinterpret_as_mut::<u16>()?, self.value, max_value)
                    }
                    BitType::U8 => {
                        gamma(channel.reinterpret_as_mut::<u8>()?, self.value, max_value)
                    }
                    d => {
                        return Err(ImageErrors::ImageOperationNotImplemented(
                            self.get_name(),
                            d
                        ))
                    }
                }
            }
        }
        #[cfg(feature = "threads")]
        {
            trace!("Running gamma correction in multithreaded mode");

            std::thread::scope(|s| {
                let mut errors = vec![];
                for channel in image.get_channels_mut(false) {
                    let t = s.spawn(|| match depth.bit_type() {
                        BitType::U16 => Ok(gamma(
                            channel.reinterpret_as_mut::<u16>()?,
                            self.value,
                            max_value
                        )),
                        BitType::U8 => Ok(gamma(
                            channel.reinterpret_as_mut::<u8>()?,
                            self.value,
                            max_value
                        )),
                        d => {
                            return Err(ImageErrors::ImageOperationNotImplemented(
                                self.get_name(),
                                d
                            ))
                        }
                    });
                    errors.push(t);
                }
                errors
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }
        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
