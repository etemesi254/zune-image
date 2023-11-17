/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
#![allow(dead_code)]
use std::fmt::Debug;

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

//use crate::spatial::spatial_mut;

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
    #[must_use]
    pub fn new(radius: usize) -> Median {
        Median { radius }
    }
}

impl OperationsTrait for Median {
    fn name(&self) -> &'static str {
        "Median Filter"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();

        let depth = image.depth();
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running median filter single threaded mode");

            for channel in image.get_channels_mut(false) {
                let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

                match depth.bit_type() {
                    BitType::U16 => median(
                        channel.reinterpret_as::<u16>()?,
                        new_channel.reinterpret_as_mut::<u16>()?,
                        self.radius,
                        width,
                        height
                    ),
                    BitType::U8 => median(
                        channel.reinterpret_as::<u8>().unwrap(),
                        new_channel.reinterpret_as_mut::<u8>()?,
                        self.radius,
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
                *channel = new_channel;
            }
        }
        #[cfg(feature = "threads")]
        {
            trace!("Running median filter multithreaded mode");

            std::thread::scope(|s| {
                let mut errors = vec![];
                for channel in image.channels_mut(true) {
                    let result = s.spawn(|| {
                        let mut new_channel =
                            Channel::new_with_bit_type(channel.len(), depth.bit_type());

                        match depth.bit_type() {
                            BitType::U16 => median(
                                channel.reinterpret_as::<u16>()?,
                                new_channel.reinterpret_as_mut::<u16>()?,
                                self.radius,
                                width,
                                height
                            ),
                            BitType::U8 => median(
                                channel.reinterpret_as::<u8>()?,
                                new_channel.reinterpret_as_mut::<u8>()?,
                                self.radius,
                                width,
                                height
                            ),
                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.name(),
                                    d
                                ))
                            }
                        }
                        *channel = new_channel;
                        Ok(())
                    });
                    errors.push(result);
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

pub fn find_median<T: Copy + Ord>(array: &mut [T]) -> T {
    array.sort_unstable();
    let middle = array.len() / 2;

    array[middle]
}

/// Median returns a new image in which each pixel is the median of its neighbors.
/// The parameter radius corresponds to the radius of the neighbor area to be searched,
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
pub fn median<T: Copy + Ord + Default + Debug>(
    _in_channel: &[T], _out_channel: &mut [T], _radius: usize, _width: usize, _height: usize
) {
    //panic!();
    //spatial_mut(in_channel, out_channel, radius, width, height, find_median);
}
