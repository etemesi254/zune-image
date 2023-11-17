/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Spatial operations on images
//!
//! spatial goes through each pixel on an image collecting its neighbors and picking one
//! based on the function provided.
//!
//! The resulting image is then returned.
//! The parameter radius corresponds to the radius of the neighbor area to be searched,
//! for example a radius of R will result in a search window length of 2R+1 for each dimension.
//!
//!

/// Spatial operations on images.
///
/// The parameter radius corresponds to the radius of the neighbor area the statistic is applied,
/// larger radius means more compute time.
///
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
pub struct SpatialOps {
    radius:    usize,
    operation: SpatialOperations
}

impl SpatialOps {
    #[must_use]
    pub fn new(radius: usize, operation: SpatialOperations) -> SpatialOps {
        SpatialOps { radius, operation }
    }
}

impl OperationsTrait for SpatialOps {
    fn name(&self) -> &'static str {
        "StatisticsOps Filter"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();

        let depth = image.depth();
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running erode filter in single threaded mode");

            for channel in image.get_channels_mut(true) {
                let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

                match depth.bit_type() {
                    BitType::U16 => spatial_ops(
                        channel.reinterpret_as::<u16>()?,
                        new_channel.reinterpret_as_mut::<u16>()?,
                        self.radius,
                        width,
                        height,
                        self.operation
                    ),
                    BitType::U8 => spatial_ops(
                        channel.reinterpret_as::<u8>()?,
                        new_channel.reinterpret_as_mut::<u8>()?,
                        self.radius,
                        width,
                        height,
                        self.operation
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
            trace!(
                "Running statistics filter  for {:?} in multithreaded mode",
                self.operation
            );

            std::thread::scope(|s| {
                let mut errors = vec![];
                for channel in image.channels_mut(false) {
                    let result = s.spawn(|| {
                        let mut new_channel =
                            Channel::new_with_bit_type(channel.len(), depth.bit_type());

                        match depth.bit_type() {
                            BitType::U16 => spatial_ops(
                                channel.reinterpret_as::<u16>()?,
                                new_channel.reinterpret_as_mut::<u16>()?,
                                self.radius,
                                width,
                                height,
                                self.operation
                            ),
                            BitType::U8 => spatial_ops(
                                channel.reinterpret_as::<u8>()?,
                                new_channel.reinterpret_as_mut::<u8>()?,
                                self.radius,
                                width,
                                height,
                                self.operation
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

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::spatial_ops::{spatial_ops, SpatialOperations};
use crate::utils::z_prefetch;

/// Go through image neighbord, execute a function on it and return the result
/// The parameter `function` is the function that receives the list of neighbors and returns the selected
/// neighbor to be used for the resulting image.
///
///
/// # Arguments
///
/// - in_channel: input channel, the width and height are padded with radius*2 edges
///   (use pad function for that). otherwise this function will panic.
///
/// - out_channel: Output channel, the width and height are not padded at all.
///
/// - radius: Area to be searched, example a radius of R will result in a search window
/// length of 2R+1 for each dimension.
///
/// - function: Any function that when given an array returns a single element.
///
pub fn spatial<T, F>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize, height: usize,
    function: F
) where
    T: Default + Copy,
    F: Fn(&[T]) -> T
{
    let old_width = width;
    let height = (radius * 2) + height;
    let width = (radius * 2) + width;

    assert_eq!(height * width, in_channel.len());

    let radius_size = (2 * radius) + 1;

    let radius_loop = radius_size >> 1;

    let mut local_storage = vec![T::default(); radius_size * radius_size];

    for y in radius_loop..height - radius_loop {
        for x in radius_loop..width - radius_loop {
            let iy = y - radius_loop;
            let ix = x - radius_loop;

            let mut i = 0;

            for ky in 0..radius_size {
                let iy_i = iy + ky;

                let in_slice = &in_channel[(iy_i * width) + ix..(iy_i * width) + ix + radius_size];
                z_prefetch(in_channel, (iy_i + 1) * width + ix);
                local_storage[i..i + radius_size].copy_from_slice(in_slice);
                z_prefetch(in_channel, (iy_i + 2) * width + ix);

                i += radius_size;
            }

            let result = function(&local_storage);

            out_channel[iy * old_width + ix] = result;
        }
    }
}

/// A special spatial function that takes advantage of const generics to
/// speed up operations for convolve
#[allow(non_snake_case)]
pub(crate) fn spatial_NxN<T, F, const RADIUS: usize, const OUT_SIZE: usize>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, function: F
) where
    T: Default + Copy,
    F: Fn(&[T; OUT_SIZE]) -> T
{
    let old_width = width;
    let height = (RADIUS * 2) + height;
    let width = (RADIUS * 2) + width;

    assert_eq!(height * width, in_channel.len());

    let radius_size = (2 * RADIUS) + 1;

    let radius_loop = radius_size >> 1;

    let mut local_storage = [T::default(); OUT_SIZE];

    for y in radius_loop..height - radius_loop {
        for x in radius_loop..width - radius_loop {
            let iy = y - radius_loop;
            let ix = x - radius_loop;

            let mut i = 0;

            for ky in 0..radius_size {
                let iy_i = iy + ky;

                let in_slice = &in_channel[(iy_i * width) + ix..(iy_i * width) + ix + radius_size];
                z_prefetch(in_channel, (iy_i + 1) * width + ix);
                local_storage[i..i + radius_size].copy_from_slice(in_slice);
                z_prefetch(in_channel, (iy_i + 2) * width + ix);

                i += radius_size;
            }

            let result = function(&local_storage);

            out_channel[iy * old_width + ix] = result;
        }
    }
}
