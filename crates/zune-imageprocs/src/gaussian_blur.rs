/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! An implementation of a gaussian-blur.
//!
//! This module implements a gaussian blur functions for images
//!
//! The implementation does not give the true gaussian coefficients of the
//! as that is an expensive operation but rather approximates it using a series of
//! box blurs
//!
//! For the math behind it see <https://blog.ivank.net/fastest-gaussian-blur.html>

use crate::mathops::{compute_mod_u32, fastdiv_u32};
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

#[derive(Default)]
pub struct GaussianBlur {
    sigma: f32,
}

impl GaussianBlur {
    /// Create a new gaussian blur filter
    ///
    /// # Arguments
    /// - sigma: How much to blur by.
    #[must_use]
    pub fn new(sigma: f32) -> GaussianBlur {
        GaussianBlur { sigma }
    }
}

impl OperationsTrait for GaussianBlur {
    fn name(&self) -> &'static str {
        "Gaussian blur"
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }

    #[allow(clippy::too_many_lines)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();
        let depth = image.depth();

        #[cfg(feature = "threads")]
        let num_threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        #[cfg(not(feature = "threads"))]
        let num_threads = 1;

        if num_threads > 1 {
            trace!("Running gaussian blur with {num_threads} spatial threads");
        } else {
            trace!("Running gaussian blur in single threaded mode");
        }

        match depth.bit_type() {
            BitType::U8 => {
                for channel in image.channels_mut(false) {
                    let mut temp = vec![0; width * height];
                    gaussian_blur_u8(
                        channel.reinterpret_as_mut::<u8>()?,
                        &mut temp,
                        width,
                        height,
                        self.sigma,
                        num_threads,
                    );
                }
            }
            BitType::U16 => {
                for channel in image.channels_mut(false) {
                    let mut temp = vec![0; width * height];
                    gaussian_blur_u16(
                        channel.reinterpret_as_mut::<u16>()?,
                        &mut temp,
                        width,
                        height,
                        self.sigma,
                        num_threads,
                    );
                }
            }
            BitType::F32 => {
                for channel in image.channels_mut(false) {
                    let mut temp = vec![0.0; width * height];
                    gaussian_blur_f32(
                        channel.reinterpret_as_mut()?,
                        &mut temp,
                        width,
                        height,
                        self.sigma,
                        num_threads,
                    );
                }
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::cast_precision_loss
)]
fn create_box_gauss(sigma: f32) -> [usize; 3] {
    let mut radii = [1_usize; 3];
    if sigma > 0.0 {
        let n_float = 3.0;
        let w_ideal = ((12.0 * sigma * sigma / n_float) + 1.0).sqrt();
        let mut wl: i32 = w_ideal.floor() as i32;

        if wl % 2 == 0 {
            wl -= 1;
        }

        let wu = (wl + 2) as usize;
        let wl_float = wl as f32;

        let m_ideal = (12.0 * sigma * sigma
            - n_float * wl_float * wl_float
            - 4.0 * n_float * wl_float
            - 3.0 * n_float)
            / (-4.0 * wl_float - 4.0);

        let m: usize = m_ideal.round() as usize;

        for i in 0..3 {
            let diameter = if i < m { wl as usize } else { wu };
            radii[i] = (diameter - 1) / 2;
        }
    }
    radii
}

pub fn gaussian_blur_u8(
    in_out_image: &mut [u8], scratch_space: &mut [u8], width: usize, height: usize, sigma: f32,
    num_threads: usize,
) {
    let blur_radii = create_box_gauss(sigma);
    assert_eq!(blur_radii.len(), 3, "Update pass operations");

    let chunk_height = (height + num_threads - 1) / num_threads.max(1);
    let chunk_size = chunk_height * width;

    #[cfg(feature = "threads")]
    let use_threads = num_threads > 1;
    #[cfg(not(feature = "threads"))]
    let use_threads = false;

    // 1. Horizontal Passes
    if use_threads {
        #[cfg(feature = "threads")]
        std::thread::scope(|s| {
            for (in_chunk, scratch_chunk) in in_out_image
                .chunks_mut(chunk_size)
                .zip(scratch_space.chunks_mut(chunk_size))
            {
                s.spawn(move || {
                    for (in_row, scratch_row) in in_chunk
                        .chunks_exact_mut(width)
                        .zip(scratch_chunk.chunks_exact_mut(width))
                    {
                        crate::box_blur::box_blur_inner(in_row, scratch_row, width, blur_radii[0]);
                        crate::box_blur::box_blur_inner(scratch_row, in_row, width, blur_radii[1]);
                        crate::box_blur::box_blur_inner(in_row, scratch_row, width, blur_radii[2]);
                    }
                });
            }
        });
    } else {
        for (in_row, scratch_row) in in_out_image
            .chunks_exact_mut(width)
            .zip(scratch_space.chunks_exact_mut(width))
        {
            crate::box_blur::box_blur_inner(in_row, scratch_row, width, blur_radii[0]);
            crate::box_blur::box_blur_inner(scratch_row, in_row, width, blur_radii[1]);
            crate::box_blur::box_blur_inner(in_row, scratch_row, width, blur_radii[2]);
        }
    }

    // 2. Vertical Passes
    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        let (input, output): (&[u8], &mut [u8]) = if pos % 2 == 0 {
            (scratch_space, in_out_image)
        } else {
            (in_out_image, scratch_space)
        };

        if use_threads {
            #[cfg(feature = "threads")]
            std::thread::scope(|s| {
                for (i, out_chunk) in output.chunks_mut(chunk_size).enumerate() {
                    let y_start = i * chunk_height;
                    let radius = *blur_radius;
                    s.spawn(move || {
                        box_blur_vertical_u8_chunk(
                            input, out_chunk, width, height, radius, y_start,
                        );
                    });
                }
            });
        } else {
            box_blur_vertical_u8_chunk(input, output, width, height, *blur_radius, 0);
        }
    }
}

pub fn gaussian_blur_u16(
    in_out_image: &mut [u16], scratch_space: &mut [u16], width: usize, height: usize, sigma: f32,
    num_threads: usize,
) {
    let blur_radii = create_box_gauss(sigma);
    let chunk_height = (height + num_threads - 1) / num_threads.max(1);
    let chunk_size = chunk_height * width;

    #[cfg(feature = "threads")]
    let use_threads = num_threads > 1;
    #[cfg(not(feature = "threads"))]
    let use_threads = false;

    if use_threads {
        #[cfg(feature = "threads")]
        std::thread::scope(|s| {
            for (in_chunk, scratch_chunk) in in_out_image
                .chunks_mut(chunk_size)
                .zip(scratch_space.chunks_mut(chunk_size))
            {
                s.spawn(move || {
                    for (in_row, scratch_row) in in_chunk
                        .chunks_exact_mut(width)
                        .zip(scratch_chunk.chunks_exact_mut(width))
                    {
                        crate::box_blur::box_blur_inner(in_row, scratch_row, width, blur_radii[0]);
                        crate::box_blur::box_blur_inner(scratch_row, in_row, width, blur_radii[1]);
                        crate::box_blur::box_blur_inner(in_row, scratch_row, width, blur_radii[2]);
                    }
                });
            }
        });
    } else {
        for (in_row, scratch_row) in in_out_image
            .chunks_exact_mut(width)
            .zip(scratch_space.chunks_exact_mut(width))
        {
            crate::box_blur::box_blur_inner(in_row, scratch_row, width, blur_radii[0]);
            crate::box_blur::box_blur_inner(scratch_row, in_row, width, blur_radii[1]);
            crate::box_blur::box_blur_inner(in_row, scratch_row, width, blur_radii[2]);
        }
    }

    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        let (input, output): (&[u16], &mut [u16]) = if pos % 2 == 0 {
            (scratch_space, in_out_image)
        } else {
            (in_out_image, scratch_space)
        };

        if use_threads {
            #[cfg(feature = "threads")]
            std::thread::scope(|s| {
                for (i, out_chunk) in output.chunks_mut(chunk_size).enumerate() {
                    let y_start = i * chunk_height;
                    let radius = *blur_radius;
                    s.spawn(move || {
                        box_blur_vertical_u16_chunk(
                            input, out_chunk, width, height, radius, y_start,
                        );
                    });
                }
            });
        } else {
            box_blur_vertical_u16_chunk(input, output, width, height, *blur_radius, 0);
        }
    }
}

pub fn gaussian_blur_f32(
    in_out_image: &mut [f32], scratch_space: &mut [f32], width: usize, height: usize, sigma: f32,
    num_threads: usize,
) {
    let blur_radii = create_box_gauss(sigma);
    let chunk_height = (height + num_threads - 1) / num_threads.max(1);
    let chunk_size = chunk_height * width;

    #[cfg(feature = "threads")]
    let use_threads = num_threads > 1;
    #[cfg(not(feature = "threads"))]
    let use_threads = false;

    if use_threads {
        #[cfg(feature = "threads")]
        std::thread::scope(|s| {
            for (in_chunk, scratch_chunk) in in_out_image
                .chunks_mut(chunk_size)
                .zip(scratch_space.chunks_mut(chunk_size))
            {
                s.spawn(move || {
                    for (in_row, scratch_row) in in_chunk
                        .chunks_exact_mut(width)
                        .zip(scratch_chunk.chunks_exact_mut(width))
                    {
                        crate::box_blur::box_blur_f32_inner(
                            in_row,
                            scratch_row,
                            width,
                            blur_radii[0],
                        );
                        crate::box_blur::box_blur_f32_inner(
                            scratch_row,
                            in_row,
                            width,
                            blur_radii[1],
                        );
                        crate::box_blur::box_blur_f32_inner(
                            in_row,
                            scratch_row,
                            width,
                            blur_radii[2],
                        );
                    }
                });
            }
        });
    } else {
        for (in_row, scratch_row) in in_out_image
            .chunks_exact_mut(width)
            .zip(scratch_space.chunks_exact_mut(width))
        {
            crate::box_blur::box_blur_f32_inner(in_row, scratch_row, width, blur_radii[0]);
            crate::box_blur::box_blur_f32_inner(scratch_row, in_row, width, blur_radii[1]);
            crate::box_blur::box_blur_f32_inner(in_row, scratch_row, width, blur_radii[2]);
        }
    }

    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        let (input, output): (&[f32], &mut [f32]) = if pos % 2 == 0 {
            (scratch_space, in_out_image)
        } else {
            (in_out_image, scratch_space)
        };

        if use_threads {
            #[cfg(feature = "threads")]
            std::thread::scope(|s| {
                for (i, out_chunk) in output.chunks_mut(chunk_size).enumerate() {
                    let y_start = i * chunk_height;
                    let radius = *blur_radius;
                    s.spawn(move || {
                        box_blur_vertical_f32_chunk(
                            input, out_chunk, width, height, radius, y_start,
                        );
                    });
                }
            });
        } else {
            box_blur_vertical_f32_chunk(input, output, width, height, *blur_radius, 0);
        }
    }
}

// ============================================================================
// CACHE-FRIENDLY & SPATIALLY AWARE VERTICAL CHUNK PROCESSORS
// ============================================================================

#[inline(always)]
fn box_blur_vertical_u8_chunk(
    input: &[u8], output_chunk: &mut [u8], width: usize, height: usize, radius: usize,
    y_start: usize,
) {
    if radius == 0 || height <= 1 {
        let start_idx = y_start * width;
        let end_idx = start_idx + output_chunk.len();
        if start_idx < input.len() {
            output_chunk.copy_from_slice(&input[start_idx..end_idx.min(input.len())]);
        }
        return;
    }

    let chunk_height = output_chunk.len() / width;
    if chunk_height == 0 {
        return;
    }

    let diameter = (radius * 2 + 1) as u32;
    let diameter = diameter.min(height as u32);
    let m_radius = compute_mod_u32(u64::from(diameter));

    let mut sums = vec![0u32; width];

    // Initialize the window precisely for y_start by pre-summing the vertical slice
    for dy in 0..=(radius * 2) {
        let real_y = if dy < radius {
            let diff = radius - dy;
            y_start.saturating_sub(diff)
        } else {
            let diff = dy - radius;
            (y_start + diff).min(height - 1)
        };
        let row = &input[real_y * width..real_y * width + width];
        for (x, &val) in row.iter().enumerate() {
            sums[x] += u32::from(val);
        }
    }

    // Write the very first row of this chunk
    let out_row = &mut output_chunk[0..width];
    for (x, sum) in sums.iter().enumerate() {
        out_row[x] = fastdiv_u32(*sum, m_radius) as u8;
    }

    // Slide window safely down the rest of the chunk
    for y in 1..chunk_height {
        let global_y = y_start + y;
        let top_y = if global_y > radius { global_y - radius - 1 } else { 0 };
        let bottom_y = (global_y + radius).min(height - 1);

        let top_row = &input[top_y * width..top_y * width + width];
        let bottom_row = &input[bottom_y * width..bottom_y * width + width];
        let out_row = &mut output_chunk[y * width..y * width + width];

        for (((sum, &top), &bottom), out) in sums
            .iter_mut()
            .zip(top_row.iter())
            .zip(bottom_row.iter())
            .zip(out_row.iter_mut())
        {
            *sum = sum.wrapping_add(u32::from(bottom)).wrapping_sub(u32::from(top));
            *out = fastdiv_u32(*sum, m_radius) as u8;
        }
    }
}

#[inline(always)]
fn box_blur_vertical_u16_chunk(
    input: &[u16], output_chunk: &mut [u16], width: usize, height: usize, radius: usize,
    y_start: usize,
) {
    if radius == 0 || height <= 1 {
        let start_idx = y_start * width;
        let end_idx = start_idx + output_chunk.len();
        if start_idx < input.len() {
            output_chunk.copy_from_slice(&input[start_idx..end_idx.min(input.len())]);
        }
        return;
    }

    let chunk_height = output_chunk.len() / width;
    if chunk_height == 0 {
        return;
    }

    let diameter = (radius * 2 + 1) as u32;
    let diameter = diameter.min(height as u32);
    let m_radius = compute_mod_u32(u64::from(diameter));

    let mut sums = vec![0u32; width];

    for dy in 0..=(radius * 2) {
        let real_y = if dy < radius {
            let diff = radius - dy;
            y_start.saturating_sub(diff)
        } else {
            let diff = dy - radius;
            (y_start + diff).min(height - 1)
        };
        let row = &input[real_y * width..real_y * width + width];
        for (x, &val) in row.iter().enumerate() {
            sums[x] += u32::from(val);
        }
    }

    let out_row = &mut output_chunk[0..width];
    for (x, sum) in sums.iter().enumerate() {
        out_row[x] = fastdiv_u32(*sum, m_radius) as u16;
    }

    for y in 1..chunk_height {
        let global_y = y_start + y;
        let top_y = if global_y > radius { global_y - radius - 1 } else { 0 };
        let bottom_y = (global_y + radius).min(height - 1);

        let top_row = &input[top_y * width..top_y * width + width];
        let bottom_row = &input[bottom_y * width..bottom_y * width + width];
        let out_row = &mut output_chunk[y * width..y * width + width];

        for (((sum, &top), &bottom), out) in sums
            .iter_mut()
            .zip(top_row.iter())
            .zip(bottom_row.iter())
            .zip(out_row.iter_mut())
        {
            *sum = sum.wrapping_add(u32::from(bottom)).wrapping_sub(u32::from(top));
            *out = fastdiv_u32(*sum, m_radius) as u16;
        }
    }
}

#[inline(always)]
fn box_blur_vertical_f32_chunk(
    input: &[f32], output_chunk: &mut [f32], width: usize, height: usize, radius: usize,
    y_start: usize,
) {
    if radius == 0 || height <= 1 {
        let start_idx = y_start * width;
        let end_idx = start_idx + output_chunk.len();
        if start_idx < input.len() {
            output_chunk.copy_from_slice(&input[start_idx..end_idx.min(input.len())]);
        }
        return;
    }

    let chunk_height = output_chunk.len() / width;
    if chunk_height == 0 {
        return;
    }

    let weight = (radius * 2 + 1) as f32;
    let inv_weight = 1.0 / weight;
    let mut sums = vec![0.0f32; width];

    for dy in 0..=(radius * 2) {
        let real_y = if dy < radius {
            let diff = radius - dy;
            y_start.saturating_sub(diff)
        } else {
            let diff = dy - radius;
            (y_start + diff).min(height - 1)
        };
        let row = &input[real_y * width..real_y * width + width];
        for (x, &val) in row.iter().enumerate() {
            sums[x] += val;
        }
    }

    let out_row = &mut output_chunk[0..width];
    for (x, sum) in sums.iter().enumerate() {
        out_row[x] = sum * inv_weight;
    }

    for y in 1..chunk_height {
        let global_y = y_start + y;
        let top_y = if global_y > radius { global_y - radius - 1 } else { 0 };
        let bottom_y = (global_y + radius).min(height - 1);

        let top_row = &input[top_y * width..top_y * width + width];
        let bottom_row = &input[bottom_y * width..bottom_y * width + width];
        let out_row = &mut output_chunk[y * width..y * width + width];

        for (((sum, &top), &bottom), out) in sums
            .iter_mut()
            .zip(top_row.iter())
            .zip(bottom_row.iter())
            .zip(out_row.iter_mut())
        {
            *sum = *sum + bottom - top;
            *out = *sum * inv_weight;
        }
    }
}
