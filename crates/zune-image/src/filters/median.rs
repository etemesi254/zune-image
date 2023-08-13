/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use log::trace;
use zune_core::bit_depth::BitType;
use zune_imageprocs::median::median;

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Median returns a new image in which each pixel is the median of its neighbors.
///
/// The parameter radius corresponds to the radius of the neighbor area to be searched,
///
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
#[derive(Default)]
pub struct Median {
    radius: usize
}

impl Median {
    pub fn new(radius: usize) -> Median {
        Median { radius }
    }
}

impl OperationsTrait for Median {
    fn get_name(&self) -> &'static str {
        "Median Filter"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.get_dimensions();

        let depth = image.get_depth();
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running median filter single threaded mode");

            for channel in image.get_channels_mut(false) {
                let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

                match depth.bit_type() {
                    BitType::U16 => median(
                        channel.reinterpret_as::<u16>().unwrap(),
                        new_channel.reinterpret_as_mut::<u16>().unwrap(),
                        self.radius,
                        width,
                        height
                    ),
                    BitType::U8 => median(
                        channel.reinterpret_as::<u8>().unwrap(),
                        new_channel.reinterpret_as_mut::<u8>().unwrap(),
                        self.radius,
                        width,
                        height
                    ),
                    _ => todo!()
                }
                *channel = new_channel;
            }
        }
        #[cfg(feature = "threads")]
        {
            trace!("Running median filter multithreaded mode");

            std::thread::scope(|s| {
                for channel in image.get_channels_mut(true) {
                    s.spawn(|| {
                        let mut new_channel =
                            Channel::new_with_bit_type(channel.len(), depth.bit_type());

                        match depth.bit_type() {
                            BitType::U16 => median(
                                channel.reinterpret_as::<u16>().unwrap(),
                                new_channel.reinterpret_as_mut::<u16>().unwrap(),
                                self.radius,
                                width,
                                height
                            ),
                            BitType::U8 => median(
                                channel.reinterpret_as::<u8>().unwrap(),
                                new_channel.reinterpret_as_mut::<u8>().unwrap(),
                                self.radius,
                                width,
                                height
                            ),
                            _ => todo!()
                        }
                        *channel = new_channel;
                    });
                }
            });
        }
        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
