/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_imageprocs::scharr::{scharr_float, scharr_int};

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Perform a scharr image derivative.
///
/// This operation calculates the gradient of the image,
/// which represents how quickly pixel values change from
/// one point to another in both the horizontal and vertical directions.
/// The magnitude and direction of the gradient can be used to detect edges in an image.
///
/// The matrix for scharr is
///
/// Gx matrix
/// ```text
///   -3, 0,  3,
///  -10, 0, 10,
///   -3, 0,  3
/// ```
/// Gy matrix
/// ```text
/// -3,-10,-3,
///  0,  0, 0,
///  3, 10, 3
/// ```
///
/// The window is a 3x3 window.
#[derive(Default, Copy, Clone)]
pub struct Scharr;

impl Scharr {
    /// Create a new scharr filter
    pub fn new() -> Scharr {
        Self::default()
    }
}

impl OperationsTrait for Scharr {
    fn get_name(&self) -> &'static str {
        "Scharr"
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.get_depth().bit_type();
        let (width, height) = image.get_dimensions();

        #[cfg(not(feature = "threads"))]
        {
            for channel in image.get_channels_mut(true) {
                let mut out_channel = Channel::new_with_bit_type(channel.len(), depth);
                match depth {
                    BitType::U8 => scharr_int::<u8>(
                        channel.reinterpret_as()?,
                        out_channel.reinterpret_as_mut()?,
                        width,
                        height
                    ),
                    BitType::U16 => scharr_int::<u16>(
                        channel.reinterpret_as()?,
                        out_channel.reinterpret_as_mut()?,
                        width,
                        height
                    ),
                    BitType::F32 => scharr_float::<f32>(
                        channel.reinterpret_as()?,
                        out_channel.reinterpret_as_mut()?,
                        width,
                        height
                    ),
                    d => {
                        return Err(ImageErrors::ImageOperationNotImplemented(
                            self.get_name(),
                            d
                        ))
                    }
                }
                *channel = out_channel;
            }
        }
        #[cfg(feature = "threads")]
        {
            std::thread::scope(|s| {
                let mut t_results = vec![];
                for channel in image.get_channels_mut(true) {
                    let result = s.spawn(|| {
                        let mut out_channel = Channel::new_with_bit_type(channel.len(), depth);
                        match depth {
                            BitType::U8 => scharr_int::<u8>(
                                channel.reinterpret_as()?,
                                out_channel.reinterpret_as_mut()?,
                                width,
                                height
                            ),
                            BitType::U16 => scharr_int::<u16>(
                                channel.reinterpret_as()?,
                                out_channel.reinterpret_as_mut()?,
                                width,
                                height
                            ),
                            BitType::F32 => scharr_float::<f32>(
                                channel.reinterpret_as()?,
                                out_channel.reinterpret_as_mut()?,
                                width,
                                height
                            ),
                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.get_name(),
                                    d
                                ))
                            }
                        }
                        *channel = out_channel;
                        Ok(())
                    });
                    t_results.push(result);
                }

                t_results
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
