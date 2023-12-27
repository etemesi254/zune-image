/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Applies a median filter of given dimensions to an image. Each output pixel is the median
//! of the pixels in a `(2 * radius + 1) * (2 * radius + 1)` kernel of pixels in the input image.
//!
//! Performs O(radius) operations per pixel
//!
//! # Algorithm
//! A simple median function can be implemented by sorting items in a window and then picking the middle value,
//! but then this has some not so good perfomance especially with large windows
//! e.g if I were calculating a radius of 37, the window would be `((2*37)+1* (2*37)+1))` -> `75 * 75` -> `5625` values
//! to sort on every window position, which no matter what computer, it's gonna be slow
//!
//!  But one thing to note is that what happens from one window to another is that we drop the rightmost values and add the
//! leftmost values
//!
//! ```text
//!┌─┬───────────┬─┐
//!│ │           │ │
//!│ │           │ │
//!│ │           │ │
//!│ │           │ │
//!└─┴───────────┴─┘
//! ▲             ▲
//! │             │
//!drop           add
//! ```
//! So we can maintain a histogram of values in our window, the histogram tells us the frequencies of values in our windows
//! on moving from one window to another, we drop the leftmost values, and add the rightmost
//!
//! while this is still expensive, it is faster than sorting
//!
//!

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::pad::{pad, PadMethod};
use crate::utils::z_prefetch;

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

        if self.radius < 2 {
            return Ok(());
        }
        let depth = image.depth();
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running median filter single threaded mode");

            for channel in image.get_channels_mut(false) {
                let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

                match depth.bit_type() {
                    BitType::U16 => median_u16(
                        channel.reinterpret_as::<u16>()?,
                        new_channel.reinterpret_as_mut::<u16>()?,
                        self.radius,
                        width,
                        height
                    ),
                    BitType::U8 => median_u8(
                        channel.reinterpret_as::<u8>()?,
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
                            BitType::U16 => median_u16(
                                channel.reinterpret_as::<u16>()?,
                                new_channel.reinterpret_as_mut::<u16>()?,
                                self.radius,
                                width,
                                height
                            ),
                            BitType::U8 => median_u8(
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
#[allow(clippy::cast_possible_truncation)]
pub fn median_u16(
    in_channel: &[u16], out_channel: &mut [u16], radius: usize, width: usize, height: usize
) {
    /*
     * Okay rico, we run a tight shift here
     *
     * The implementation is simple, we use nested lambdas, something dope about them
     * is that they can modify parameters of the parent function, which we want.
     *
     * spatial_mut has a fixed definition, in that it wants a function that just accepts a slice, but what about histograms
     * that need to be preserved across iterations, yep why the need for lambdas
     */

    // array containing our histogram for each median window
    let mut histogram_arr = vec![0_u32; 65536];
    // give it a fixed size for optimizer to remove bounds check
    let histogram: &mut [u32; 65536] = histogram_arr.get_mut(..).unwrap().try_into().unwrap();

    let radius_size = (2 * radius) + 1;

    // vector containing items that will be dropped from the
    // histogram in the next iteration
    let mut to_be_dropped = vec![0; radius_size];
    // the current position of our window, used for handling edges and
    // knowing when we went to another row
    let mut counter = 0;

    let func = |array: &[u16]| -> u16 {
        // the position of our median
        let median_pos = array.len() / 2;

        if (counter % width) == 0 {
            // when we are starting a new row

            // zero out histogram
            histogram.fill(0);
            // add everything
            for c in array {
                histogram[usize::from(*c)] += 1;
            }
        } else {
            // drop items that fell off from previous run
            for x in &to_be_dropped {
                histogram[usize::from(*x)] -= 1;
            }
            // add the new window values added to the histogram
            // these are the rightmost values
            array.chunks_exact(radius_size).for_each(|v| {
                let to_add = usize::from(*v.last().unwrap());
                histogram[to_add] += 1;
            });
        }

        // iterate through our histogram to find the median
        let mut accum = 0;
        let mut median = 0;

        for (pos, v) in histogram.iter().enumerate() {
            accum += *v;
            if accum >= median_pos as u32 {
                // we found the median
                median = pos as u16;
                break;
            }
        }
        counter += 1;
        // set up for our next loop
        // add the things to be dropped in since they fell off our window in the to_be_dropped
        // in the next iteration, they will be subtracted from the histogram

        debug_assert!(array.chunks_exact(radius_size).len() == to_be_dropped.len());
        array
            .chunks_exact(radius_size)
            .zip(to_be_dropped.iter_mut())
            .for_each(|(x, v)| {
                *v = x[0];
            });
        median
    };

    // pad input
    let padded_input = pad(
        in_channel,
        width,
        height,
        radius,
        radius,
        PadMethod::Replicate
    );
    spatial_median(&padded_input, out_channel, radius, width, height, func);
}
#[allow(clippy::cast_possible_truncation)]
pub fn median_u8(
    in_channel: &[u8], out_channel: &mut [u8], radius: usize, width: usize, height: usize
) {
    // duplicated from above, but uses array instead of vec, and

    // array containing our histogram for each median window
    let mut histogram = [0_u32; 256];

    let radius_size = (2 * radius) + 1;

    // vector containing items that will be dropped from the
    // histogram in the next iteration
    let mut to_be_dropped = vec![0; radius_size];
    // the current position of our window, used for handling edges and
    // knowing when we went to another row
    let mut counter = 0;

    let func = |array: &[u8]| -> u8 {
        // the position of our median
        let median_pos = array.len() / 2;

        if (counter % width) == 0 {
            // when we are starting a new row

            // zero out histogram
            histogram.fill(0);
            // add everything
            for c in array {
                histogram[usize::from(*c)] += 1;
            }
        } else {
            // drop items that fell off from previous run
            for x in &to_be_dropped {
                histogram[usize::from(*x)] -= 1;
            }
            // add the new window values added to the histogram
            // these are the rightmost values
            array.chunks_exact(radius_size).for_each(|v| {
                let to_add = usize::from(*v.last().unwrap());
                histogram[to_add] += 1;
            });
        }

        // iterate through our histogram to find the median
        let mut accum = 0;
        let mut median = 0;

        for (pos, v) in histogram.iter().enumerate() {
            accum += *v;
            if accum >= median_pos as u32 {
                // we found the median
                median = pos as u8;
                break;
            }
        }
        counter += 1;
        // set up for our next loop
        // add the things to be dropped in since they fell off our window in the to_be_dropped
        // in the next iteration, they will be subtracted from the histogram

        debug_assert!(array.chunks_exact(radius_size).len() == to_be_dropped.len());
        array
            .chunks_exact(radius_size)
            .zip(to_be_dropped.iter_mut())
            .for_each(|(x, v)| {
                *v = x[0];
            });
        median
    };

    // pad input
    let padded_input = pad(
        in_channel,
        width,
        height,
        radius,
        radius,
        PadMethod::Replicate
    );
    spatial_median(&padded_input, out_channel, radius, width, height, func);
}

pub fn spatial_median<T, F>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize, height: usize,
    mut function: F
) where
    T: Default + Copy,
    F: FnMut(&[T]) -> T
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

                z_prefetch(in_channel, (iy_i + 1) * width + ix);
                let in_slice = &in_channel[(iy_i * width) + ix..(iy_i * width) + ix + radius_size];
                local_storage[i..i + radius_size].copy_from_slice(in_slice);
                z_prefetch(in_channel, (iy_i + 2) * width + ix);

                i += radius_size;
            }

            let result = function(&local_storage);

            out_channel[iy * old_width + ix] = result;
        }
    }
}
