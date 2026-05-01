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
use std::thread;

/// Spatial operations on images.
///
/// The parameter radius corresponds to the radius of the neighbor area the statistic is applied,
/// larger radius means more compute time.
///
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
pub struct SpatialOps {
    radius: usize,
    operation: SpatialOperations,
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

        let spatial_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

            match depth.bit_type() {
                BitType::U16 => spatial_ops(
                    channel.reinterpret_as::<u16>()?,
                    new_channel.reinterpret_as_mut::<u16>()?,
                    self.radius,
                    width,
                    height,
                    self.operation,
                ),
                BitType::U8 => spatial_ops(
                    channel.reinterpret_as::<u8>()?,
                    new_channel.reinterpret_as_mut::<u8>()?,
                    self.radius,
                    width,
                    height,
                    self.operation,
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }
            *channel = new_channel;
            Ok(())
        };

        execute_on(spatial_fn, image, true)
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}

use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::spatial_ops::{spatial_ops, SpatialOperations};
use crate::utils::{execute_on, z_prefetch};

/// Go through image neighbourhood, execute a function on it and return the result
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
#[cfg(not(feature = "threads"))]
pub fn spatial<T, F>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize, height: usize,
    function: F,
) where
    T: Default + Copy,
    F: Fn(&[T]) -> T,
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

#[cfg(feature = "threads")]
pub fn spatial<T, F>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize, height: usize,
    function: F,
) where
    T: Default + Copy + Send + Sync, // Added Send + Sync
    F: Fn(&[T]) -> T + Sync,         // Added Sync for the function pointer
{
    let old_width = width;
    let orig_height = height; // The actual unpadded height
    let padded_height = (radius * 2) + height;
    let padded_width = (radius * 2) + width;

    assert_eq!(padded_height * padded_width, in_channel.len());
    assert_eq!(orig_height * old_width, out_channel.len());

    let radius_size = (2 * radius) + 1;
    let radius_loop = radius_size >> 1;

    // Determine parallel threads (fallback to 4 if OS check fails)
    let num_threads = thread::available_parallelism().map_or(4, std::num::NonZero::get);
    // Calculate how many rows each thread should handle
    // We divide the workload by rows (Y-axis) to maximize contiguous memory writes
    let rows_per_thread = orig_height.div_ceil(num_threads);
    let chunk_size = rows_per_thread * old_width;

    thread::scope(|s| {
        // Safe Rust magic: split the mutable output into non-overlapping chunks
        for (chunk_idx, out_chunk) in out_channel.chunks_mut(chunk_size).enumerate() {
            // Borrow immutably for the thread
            let in_channel_ref = &in_channel;
            let function_ref = &function;

            s.spawn(move || {
                // Each thread gets its own local storage
                let mut local_storage = vec![T::default(); radius_size * radius_size];

                let iy_start = chunk_idx * rows_per_thread;
                let chunk_rows = out_chunk.len() / old_width;

                for local_iy in 0..chunk_rows {
                    let iy = iy_start + local_iy;
                    let y = iy + radius_loop;

                    for x in radius_loop..padded_width - radius_loop {
                        let ix = x - radius_loop;
                        let mut i = 0;

                        for ky in 0..radius_size {
                            let iy_i = iy + ky;

                            let in_slice = &in_channel_ref[(iy_i * padded_width) + ix
                                ..(iy_i * padded_width) + ix + radius_size];

                            z_prefetch(in_channel_ref, (iy_i + 1) * padded_width + ix);
                            local_storage[i..i + radius_size].copy_from_slice(in_slice);
                            z_prefetch(in_channel_ref, (iy_i + 2) * padded_width + ix);

                            i += radius_size;
                        }

                        let result = function_ref(&local_storage);

                        // Write to the local chunk starting at index 0 for this thread's workload
                        out_chunk[local_iy * old_width + ix] = result;
                    }
                }
            });
        }
    });
}
/// A special spatial function that takes advantage of const generics to
/// speed up operations for convolve
#[allow(non_snake_case)]
pub(crate) fn spatial_NxN<T, F, const RADIUS: usize, const OUT_SIZE: usize>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, function: F,
) where
    T: Default + Copy,
    F: Fn(&[T; OUT_SIZE]) -> T,
{
    // CAE: Threaded version did not work
    // https://gist.github.com/etemesi254/8e7863802816d2701c6fd87053293f71
    //
    // ---------- with multithreaded ----------------
    // imageprocs: sobel/zune-image
    //                         time:   [166.84 ms 168.53 ms 170.56 ms]
    //                         thrpt:  [7.1249 MiB/s 7.2106 MiB/s 7.2836 MiB/s]
    // ----------- single threaded -------------
    // imageprocs: sobel/zune-image
    //                         time:   [49.648 ms 50.218 ms 50.862 ms]
    //                         thrpt:  [23.892 MiB/s 24.198 MiB/s 24.476 MiB/s]

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
