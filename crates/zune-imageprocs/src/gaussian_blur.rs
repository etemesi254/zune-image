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
use zune_image::planar_regions::{PlanarRegionMut, PlanarRegionOut};
/// Applies a fast Gaussian blur to the image.
///
/// A true Gaussian blur is computationally expensive because it requires convolving the
/// image with a large, mathematically precise bell-curve matrix.
///
/// # Algorithm
///
/// This implementation uses a highly optimized approximation algorithm. Based on the
/// Central Limit Theorem, applying several sequential box blurs mathematically approaches
/// a true Gaussian distribution.
///
/// This filter calculates the ideal box-blur radii for three separate passes to approximate
/// the requested Gaussian standard deviation (`sigma`). Because box blurs operate in $O(1)$
/// time relative to their radius, this makes the Gaussian blur extremely fast, even for
/// massive blur radii.
///
/// *Reference: [Fastest Gaussian Blur by Ivan Kutskir](https://blog.ivank.net/fastest-gaussian-blur.html)*
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::gaussian_blur::GaussianBlur;
/// use zune_image::errors::ImageErrors;
///
/// let mut img = Image::fill(255_u8, ColorSpace::RGB, 100, 100);
///
/// // Apply a Gaussian blur with a sigma of 5.0
/// let blur = GaussianBlur::new(5.0);
/// blur.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();
        trace!("Running gaussian blur using parallel regions");

        let radii = create_box_gauss(self.sigma);

        // Allocate our single scratch image upfront
        let mut scratch = image.clone();

        // TODO: CAE investigate blurs and alpha channel
        let ignore_alpha = false;

        match depth.bit_type() {
            BitType::U8 => {
                // 1. Horizontal Passes (In-Place)
                image.par_process_regions::<u8, _>(ignore_alpha, |region| {
                    horizontal_blur_region_u8(region, &radii);
                })?;

                // 2. Vertical Passes (Out-of-Place Ping-Pong!)
                // Pass 1: Image -> Scratch
                image.par_process_regions_out_of_place::<u8, _>(&mut scratch, ignore_alpha, |region| {
                    vertical_blur_region_u8(region, radii[0]);
                })?;

                // Pass 2: Scratch -> Image
                scratch.par_process_regions_out_of_place::<u8, _>(image, ignore_alpha, |region| {
                    vertical_blur_region_u8(region, radii[1]);
                })?;

                // Pass 3: Image -> Scratch
                image.par_process_regions_out_of_place::<u8, _>(&mut scratch, ignore_alpha, |region| {
                    vertical_blur_region_u8(region, radii[2]);
                })?;
            }
            BitType::U16 => {
                image.par_process_regions::<u16, _>(ignore_alpha, |region| {
                    horizontal_blur_region_u16(region, &radii);
                })?;

                image.par_process_regions_out_of_place::<u16, _>(&mut scratch, ignore_alpha, |region| vertical_blur_region_u16(region, radii[0]))?;
                scratch.par_process_regions_out_of_place::<u16, _>(image, ignore_alpha, |region| vertical_blur_region_u16(region, radii[1]))?;
                image.par_process_regions_out_of_place::<u16, _>(&mut scratch, ignore_alpha, |region| vertical_blur_region_u16(region, radii[2]))?;
            }
            BitType::F32 => {
                image.par_process_regions::<f32, _>(ignore_alpha, |region| {
                    horizontal_blur_region_f32(region, &radii);
                })?;

                image.par_process_regions_out_of_place::<f32, _>(&mut scratch, ignore_alpha, |region| vertical_blur_region_f32(region, radii[0]))?;
                scratch.par_process_regions_out_of_place::<f32, _>(image, ignore_alpha, |region| vertical_blur_region_f32(region, radii[1]))?;
                image.par_process_regions_out_of_place::<f32, _>(&mut scratch, ignore_alpha, |region| vertical_blur_region_f32(region, radii[2]))?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }

        // The final result of the 3rd vertical pass ends up in the scratch buffer.
        *image = scratch;

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

// --- HORIZONTAL ADAPTERS (IN-PLACE) ---

fn horizontal_blur_region_u8(region: &mut PlanarRegionMut<'_, u8>, radii: &[usize; 3]) {
    let width = region.width;
    // Tiny, L1-cache friendly scratch space local to this thread
    let mut scratch_row = vec![0u8; width];

    for channel in region.channels.iter_mut() {
        for row in channel.chunks_exact_mut(width) {
            crate::box_blur::box_blur_inner(row, &mut scratch_row, width, radii[0]);
            crate::box_blur::box_blur_inner(&scratch_row, row, width, radii[1]);
            crate::box_blur::box_blur_inner(row, &mut scratch_row, width, radii[2]);
            // Final result is in scratch_row, copy back
            row.copy_from_slice(&scratch_row);
        }
    }
}

fn horizontal_blur_region_u16(region: &mut PlanarRegionMut<'_, u16>, radii: &[usize; 3]) {
    let width = region.width;
    let mut scratch_row = vec![0u16; width];

    for channel in region.channels.iter_mut() {
        for row in channel.chunks_exact_mut(width) {
            crate::box_blur::box_blur_inner(row, &mut scratch_row, width, radii[0]);
            crate::box_blur::box_blur_inner(&mut scratch_row, row, width, radii[1]);
            crate::box_blur::box_blur_inner(row, &mut scratch_row, width, radii[2]);
            row.copy_from_slice(&scratch_row);
        }
    }
}

fn horizontal_blur_region_f32(region: &mut PlanarRegionMut<'_, f32>, radii: &[usize; 3]) {
    let width = region.width;
    let mut scratch_row = vec![0.0f32; width];

    for channel in region.channels.iter_mut() {
        for row in channel.chunks_exact_mut(width) {
            crate::box_blur::box_blur_f32_inner(row, &mut scratch_row, width, radii[0]);
            crate::box_blur::box_blur_f32_inner(&mut scratch_row, row, width, radii[1]);
            crate::box_blur::box_blur_f32_inner(row, &mut scratch_row, width, radii[2]);
            row.copy_from_slice(&scratch_row);
        }
    }
}


// --- VERTICAL ADAPTERS (OUT-OF-PLACE) ---

fn vertical_blur_region_u8(region: &mut PlanarRegionOut<'_, u8>, radius: usize) {
    if region.src_channels.is_empty() { return; }
    let width = region.width;
    let height = region.src_channels[0].len() / width;

    for (src, dest) in region.src_channels.iter().zip(region.dest_channels.iter_mut()) {
        box_blur_vertical_u8_chunk(src, dest, width, height, radius, region.y_offset);
    }
}

fn vertical_blur_region_u16(region: &mut PlanarRegionOut<'_, u16>, radius: usize) {
    if region.src_channels.is_empty() { return; }
    let width = region.width;
    let height = region.src_channels[0].len() / width;

    for (src, dest) in region.src_channels.iter().zip(region.dest_channels.iter_mut()) {
        box_blur_vertical_u16_chunk(src, dest, width, height, radius, region.y_offset);
    }
}

fn vertical_blur_region_f32(region: &mut PlanarRegionOut<'_, f32>, radius: usize) {
    if region.src_channels.is_empty() { return; }
    let width = region.width;
    let height = region.src_channels[0].len() / width;

    for (src, dest) in region.src_channels.iter().zip(region.dest_channels.iter_mut()) {
        box_blur_vertical_f32_chunk(src, dest, width, height, radius, region.y_offset);
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

    let chunk_height = output_chunk.len() / width;
    if chunk_height == 0 {
        return;
    }

    let weight = (radius * 2 + 1) as f64; // Use f64
    let inv_weight = 1.0 / weight;

    // 1. Allocate sums as f64
    let mut sums = vec![0.0f64; width];

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
            sums[x] += f64::from(val); // Accumulate as f64
        }
    }

    let out_row = &mut output_chunk[0..width];
    for (x, sum) in sums.iter().enumerate() {
        out_row[x] = (sum * inv_weight) as f32; // Cast down
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
            // Calculate with f64 precision
            *sum = *sum + f64::from(bottom) - f64::from(top);
            *out = (*sum * inv_weight) as f32; // Cast down
        }
    }
}
