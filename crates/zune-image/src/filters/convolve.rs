/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(dead_code)]

use zune_core::log::trace;
use zune_core::bit_depth::BitType;
use zune_imageprocs::convolve::convolve;

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Convolve an image
#[derive(Default)]
pub struct Convolve {
    weights: Vec<f32>
}

impl Convolve {
    /// Create a new convolve matrix, this supports 3x3,5x5 and 7x7 matrices
    pub fn new(weights: Vec<f32>) -> Convolve {
        Convolve { weights }
    }
}

impl OperationsTrait for Convolve {
    fn get_name(&self) -> &'static str {
        "2D convolution"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.get_dimensions();
        let depth = image.get_depth();

        #[cfg(feature = "threads")]
        {
            trace!("Running convolve in multithreaded mode");

            std::thread::scope(|s| {
                for channel in image.get_channels_mut(true) {
                    s.spawn(|| {
                        // Hello
                        let mut out_channel = Channel::new_with_bit_type(
                            width * height * depth.size_of(),
                            depth.bit_type()
                        );

                        match depth.bit_type() {
                            BitType::U8 => {
                                convolve(
                                    channel.reinterpret_as::<u8>().unwrap(),
                                    out_channel.reinterpret_as_mut::<u8>().unwrap(),
                                    width,
                                    height,
                                    &self.weights
                                );
                                *channel = out_channel;
                            }
                            BitType::U16 => {
                                convolve(
                                    channel.reinterpret_as::<u16>().unwrap(),
                                    out_channel.reinterpret_as_mut::<u16>().unwrap(),
                                    width,
                                    height,
                                    &self.weights
                                );
                                *channel = out_channel;
                            }
                            BitType::F32 => {
                                convolve(
                                    channel.reinterpret_as::<f32>().unwrap(),
                                    out_channel.reinterpret_as_mut::<f32>().unwrap(),
                                    width,
                                    height,
                                    &self.weights
                                );
                                *channel = out_channel;
                            }
                            _ => todo!()
                        }
                    });
                }
            });
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
