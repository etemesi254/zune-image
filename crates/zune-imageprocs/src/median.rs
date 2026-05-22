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
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;
use zune_image::traits::OperationsTrait;

/// Applies a median filter of a given radius to the image.
///
/// A median filter replaces each pixel with the median value of its neighboring pixels
/// within a `(2 * radius + 1) x (2 * radius + 1)` window. This is highly effective at
/// removing "salt and pepper" noise while preserving sharp edges, unlike blur filters
/// which soften edges.
///
/// # Algorithm
///
/// A naive median filter sorts the entire window for every pixel, which is extremely
/// slow for large radii. This implementation uses a sliding histogram approach:
/// As the window moves right by one pixel, it drops the leftmost column from the
/// histogram and adds the new rightmost column. This reduces the theoretical
/// operation complexity per pixel.
///
/// # Limitations
///
/// Because this relies on discrete integer histograms, it is only supported for `U8`
/// and `U16` bit depths. Floating-point (`F32`) images are not supported.
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::median::Median;
/// use zune_image::errors::ImageErrors;
///
/// // Create an image
/// let mut img = Image::fill(128_u8, ColorSpace::RGB, 100, 100);
///
/// // Apply a median filter with a radius of 3 (7x7 window)
/// let median = Median::new(3);
/// median.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Default)]
pub struct Median {
    radius: usize,
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
        if self.radius < 1 {
            return Ok(());
        }

        let depth = image.depth();
        trace!("Running median filter using par_process_regions_out_of_place");

        // Pre-allocate the destination buffer
        let mut dest_image = image.clone();

        // Median usually ignores alpha channel
        let ignore_alpha = true;

        match depth.bit_type() {
            BitType::U8 => {
                image.par_process_regions_out_of_place::<u8, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| median_region_u8(region, self.radius),
                )?;
            }
            BitType::U16 => {
                image.par_process_regions_out_of_place::<u16, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| median_region_u16(region, self.radius),
                )?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }

        // Overwrite the original image
        *image = dest_image;
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}

struct HistogramU8 {
    counts: [u32; 256],
    total: u32,
}

impl HistogramU8 {
    #[inline(always)]
    fn new() -> Self {
        Self {
            counts: [0; 256],
            total: 0,
        }
    }

    #[inline(always)]
    fn add(&mut self, val: u8) {
        self.counts[val as usize] += 1;
        self.total += 1;
    }

    #[inline(always)]
    fn remove(&mut self, val: u8) {
        self.counts[val as usize] -= 1;
        self.total -= 1;
    }

    #[inline(always)]
    fn find_median(&self) -> u8 {
        let target = self.total / 2;
        let mut accum = 0;
        for (i, &c) in self.counts.iter().enumerate() {
            accum += c;
            if accum > target {
                return i as u8;
            }
        }
        255
    }
}

struct HistogramU16 {
    coarse: [u32; 256],
    fine: [u32; 65536],
    total: u32,
}

impl HistogramU16 {
    #[inline(always)]
    fn new() -> Self {
        Self {
            coarse: [0; 256],
            fine: [0; 65536],
            total: 0,
        }
    }

    #[inline(always)]
    fn add(&mut self, val: u16) {
        self.coarse[(val >> 8) as usize] += 1;
        self.fine[val as usize] += 1;
        self.total += 1;
    }

    #[inline(always)]
    fn remove(&mut self, val: u16) {
        self.coarse[(val >> 8) as usize] -= 1;
        self.fine[val as usize] -= 1;
        self.total -= 1;
    }

    #[inline(always)]
    fn find_median(&self) -> u16 {
        let target = self.total / 2;
        let mut accum = 0;

        // 1. Find the coarse bucket
        let mut coarse_idx = 0;
        for (i, &c) in self.coarse.iter().enumerate() {
            if accum + c > target {
                coarse_idx = i;
                break;
            }
            accum += c;
        }

        // 2. Scan the 256 items in that specific fine bucket
        let start = coarse_idx * 256;
        for i in start..(start + 256) {
            accum += self.fine[i];
            if accum > target {
                return i as u16;
            }
        }
        65535
    }
}

fn median_region_u8(region: &mut PlanarRegionOut<'_, u8>, radius: usize) {
    let width = region.width;
    if region.src_channels.is_empty() || width == 0 {
        return;
    }

    // Derive global height from the full source slice
    let global_height = region.src_channels[0].len() / width;

    let r_isize = radius.cast_signed();
    let w_isize = width.cast_signed();
    let h_isize = global_height.cast_signed();

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;
            let mut hist = HistogramU8::new();

            // 1. Initialize the histogram for the very first pixel in the row (x = 0)
            for dy in -r_isize..=r_isize {
                let real_y = (global_y.cast_signed() + dy).clamp(0, h_isize - 1) as usize;
                let row_offset = real_y * width;

                for dx in -r_isize..=r_isize {
                    let real_x = dx.clamp(0, w_isize - 1) as usize;
                    hist.add(src[row_offset + real_x]);
                }
            }
            dest[local_y * width] = hist.find_median();

            // 2. Slide the window to the right
            for x in 1..width {
                let drop_x = (x.cast_signed() - r_isize - 1).clamp(0, w_isize - 1) as usize;
                let add_x = (x.cast_signed() + r_isize).clamp(0, w_isize - 1) as usize;

                // Add the new rightmost column, drop the old leftmost column
                for dy in -r_isize..=r_isize {
                    let real_y = (global_y.cast_signed() + dy).clamp(0, h_isize - 1) as usize;
                    let row_offset = real_y * width;

                    hist.remove(src[row_offset + drop_x]);
                    hist.add(src[row_offset + add_x]);
                }

                dest[local_y * width + x] = hist.find_median();
            }
        }
    }
}

fn median_region_u16(region: &mut PlanarRegionOut<'_, u16>, radius: usize) {
    let width = region.width;
    if region.src_channels.is_empty() || width == 0 {
        return;
    }

    let global_height = region.src_channels[0].len() / width;

    let r_isize = radius.cast_signed();
    let w_isize = width.cast_signed();
    let h_isize = global_height.cast_signed();

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;
            let mut hist = HistogramU16::new();

            // 1. Initialize the histogram for the very first pixel in the row (x = 0)
            for dy in -r_isize..=r_isize {
                let real_y = (global_y as isize + dy).clamp(0, h_isize - 1) as usize;
                let row_offset = real_y * width;

                for dx in -r_isize..=r_isize {
                    let real_x = dx.clamp(0, w_isize - 1) as usize;
                    hist.add(src[row_offset + real_x]);
                }
            }
            dest[local_y * width] = hist.find_median();

            // 2. Slide the window to the right
            for x in 1..width {
                let drop_x = (x.cast_signed() - r_isize - 1).clamp(0, w_isize - 1) as usize;
                let add_x = (x.cast_signed() + r_isize).clamp(0, w_isize - 1) as usize;

                for dy in -r_isize..=r_isize {
                    let real_y = (global_y.cast_signed() + dy).clamp(0, h_isize - 1) as usize;
                    let row_offset = real_y * width;

                    hist.remove(src[row_offset + drop_x]);
                    hist.add(src[row_offset + add_x]);
                }

                dest[local_y * width + x] = hist.find_median();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zune_core::colorspace::ColorSpace;
    use zune_image::image::Image;
    use zune_image::traits::OperationsTrait;

    #[test]
    fn test_median_flat_image() {
        // A flat image should remain completely unchanged
        let width = 10;
        let height = 10;
        let mut img_u8 = Image::fill::<u8>(128, ColorSpace::Luma, width, height);
        let mut img_u16 = Image::fill::<u16>(32768, ColorSpace::Luma, width, height);

        Median::new(2).execute(&mut img_u8).unwrap();
        Median::new(2).execute(&mut img_u16).unwrap();

        img_u8
            .iter_pixels::<u8, _>(|_, _, px| {
                assert_eq!(px[0], 128);
            })
            .unwrap();

        img_u16
            .iter_pixels::<u16, _>(|_, _, px| {
                assert_eq!(px[0], 32768);
            })
            .unwrap();
    }
}