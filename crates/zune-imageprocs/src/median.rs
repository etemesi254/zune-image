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
/// use zune_imageprocs::median::MedianBlur;
/// use zune_image::errors::ImageErrors;
///
/// // Create an image
/// let mut img = Image::fill(128_u8, ColorSpace::RGB, 100, 100);
///
/// // Apply a median filter with a radius of 3 (7x7 window)
/// let median = MedianBlur::new(3);
/// median.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Default)]
pub struct MedianBlur {
    radius: usize,
}

impl MedianBlur {
    #[must_use]
    pub fn new(radius: usize) -> MedianBlur {
        MedianBlur { radius }
    }
}

impl OperationsTrait for MedianBlur {
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
    coarse: [u32; 16], // coarse[i] = sum of counts[i*16 .. i*16+16]
    total: u32,
    median: u8,
    below: u32, // count of values strictly below `median`
}

impl HistogramU8 {
    #[inline(always)]
    fn new() -> Self {
        Self {
            counts: [0; 256],
            coarse: [0; 16],
            total: 0,
            median: 0,
            below: 0,
        }
    }

    #[inline(always)]
    fn add(&mut self, val: u8) {
        self.counts[val as usize] += 1;
        self.coarse[val as usize >> 4] += 1;
        self.total += 1;
        if val < self.median {
            self.below += 1;
        }
        // self.rebalance();
    }

    #[inline(always)]
    fn remove(&mut self, val: u8) {
        self.counts[val as usize] -= 1;
        self.coarse[val as usize >> 4] -= 1;
        self.total -= 1;
        if val < self.median {
            self.below -= 1;
        }
        // self.rebalance();
    }

    #[inline(always)]
    fn rebalance(&mut self) {
        let target = self.total / 2;

        if self.below > target {
            // Median has drifted too high — walk DOWN
            // Fine: step back within current coarse bin
            loop {
                if self.median == 0 {
                    break;
                }
                if self.below <= target {
                    break;
                }
                self.median -= 1;
                self.below -= self.counts[self.median as usize];
            }
        } else {
            // Median may need to go UP
            loop {
                let at_median = self.counts[self.median as usize];
                if self.below + at_median > target {
                    break; // median bucket straddles the target rank
                }
                if self.median == 255 {
                    break;
                }

                // Coarse skip: if we're at the start of a coarse bin and the
                // entire bin lies below the target, skip all 16 at once
                let fine_idx = self.median as usize & 0xF;
                if fine_idx == 0 {
                    let coarse_idx = self.median as usize >> 4;
                    let bin_sum = self.coarse[coarse_idx];
                    if self.below + bin_sum <= target {
                        // Entire coarse bin is below target — skip it
                        self.below += bin_sum;
                        // Advance median to start of next coarse bin
                        // Saturating to stay in [0, 255]
                        self.median = ((coarse_idx + 1) * 16).min(255) as u8;
                        continue;
                    }
                }

                // Fine step: advance one bucket
                self.below += at_median;
                self.median += 1;
            }
        }
    }

    #[inline(always)]
    fn find_median(&mut self) -> u8 {
        self.rebalance();
        self.median
    }
}

fn median_region_u8(region: &mut PlanarRegionOut<'_, u8>, radius: usize) {
    let width = region.width;
    if region.src_channels.is_empty() || width == 0 {
        return;
    }

    let global_height = region.src_channels[0].len() / width;
    let r_isize = radius.cast_signed();
    let h_isize = global_height.cast_signed();

    // Calculate unified safe boundaries for BOTH left-to-right and right-to-left passes
    let left_end = (radius + 1).min(width);
    let right_start = width.saturating_sub(radius + 1).max(left_end);
    let kernel_h = 2 * radius + 1;

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        let mut hist = HistogramU8::new();
        let mut row_offsets = vec![0usize; kernel_h];

        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;

            // 1. Precompute row offsets for the current row
            for (i, dy) in (-r_isize..=r_isize).enumerate() {
                let real_y = (global_y.cast_signed() + dy).clamp(0, h_isize - 1) as usize;
                row_offsets[i] = real_y * width;
            }
            let row_offsets = row_offsets.as_slice();

            // 2. Vertical Initializer / Slide Down
            if local_y == 0 {
                // Absolute startup: fill the initial window at (x=0, y=0)
                for &row_off in row_offsets {
                    for dx in -r_isize..=r_isize {
                        let real_x = dx.clamp(0, width.cast_signed() - 1) as usize;
                        hist.add(src[row_off + real_x]);
                    }
                }
                dest[local_y * width] = hist.find_median();
            } else {
                // Slide down exactly 1 pixel
                // If previous row was Even, we ended at Right edge. Slide down at width - 1.
                // If previous row was Odd, we ended at Left edge. Slide down at 0.
                let x = if local_y % 2 == 1 { width - 1 } else { 0 };

                let old_top_y = (global_y.cast_signed() - 1 - r_isize).clamp(0, h_isize - 1) as usize;
                let new_bottom_y = (global_y.cast_signed() + r_isize).clamp(0, h_isize - 1) as usize;

                let old_top_off = old_top_y * width;
                let new_bottom_off = new_bottom_y * width;

                for dx in -r_isize..=r_isize {
                    let real_x = (x.cast_signed() + dx).clamp(0, width.cast_signed() - 1) as usize;
                    hist.remove(src[old_top_off + real_x]);
                    hist.add(src[new_bottom_off + real_x]);
                }
                dest[local_y * width + x] = hist.find_median();
            }

            // 3. Horizontal Snake Pass
            if local_y % 2 == 0 {

                // Left Edge
                for x in 1..left_end {
                    let drop_x = 0;
                    let add_x = (x + radius).min(width - 1);
                    for &row_off in row_offsets {
                        hist.remove(src[row_off + drop_x]);
                        hist.add(src[row_off + add_x]);
                    }
                    dest[local_y * width + x] = hist.find_median();
                }

                // Middle (Fast Path - Branchless)
                for x in left_end..right_start {
                    let drop_x = x - radius - 1;
                    let add_x = x + radius;
                    for &row_off in row_offsets {
                        hist.remove(src[row_off + drop_x]);
                        hist.add(src[row_off + add_x]);
                    }
                    dest[local_y * width + x] = hist.find_median();
                }

                // Right Edge
                for x in right_start..width {
                    let drop_x = x.saturating_sub(radius + 1);
                    let add_x = width - 1;
                    for &row_off in row_offsets {
                        hist.remove(src[row_off + drop_x]);
                        hist.add(src[row_off + add_x]);
                    }
                    dest[local_y * width + x] = hist.find_median();
                }

            } else {

                // Right Edge (x starts at width - 2, because width - 1 was done by slide_down)
                for x in (right_start..width - 1).rev() {
                    let drop_x = width - 1;
                    let add_x = x.saturating_sub(radius);
                    for &row_off in row_offsets {
                        hist.remove(src[row_off + drop_x]);
                        hist.add(src[row_off + add_x]);
                    }
                    dest[local_y * width + x] = hist.find_median();
                }

                // Middle (Fast Path - Branchless)
                for x in (left_end..right_start).rev() {
                    let drop_x = x + radius + 1;
                    let add_x = x - radius;
                    for &row_off in row_offsets {
                        hist.remove(src[row_off + drop_x]);
                        hist.add(src[row_off + add_x]);
                    }
                    dest[local_y * width + x] = hist.find_median();
                }

                // Left Edge
                for x in (0..left_end).rev() {
                    let drop_x = (x + radius + 1).min(width - 1);
                    let add_x = 0;
                    for &row_off in row_offsets {
                        hist.remove(src[row_off + drop_x]);
                        hist.add(src[row_off + add_x]);
                    }
                    dest[local_y * width + x] = hist.find_median();
                }
            }
        }
    }
}
struct HistogramU16 {
    counts: [u32; 65536],
    coarse: [u32; 256], // coarse[i] = sum of counts[i*256 .. i*256+256]
    total: u32,
    median: u16,
    below: u32, // count of values strictly below `median`
}

impl HistogramU16 {
    #[inline(always)]
    fn new() -> Self {
        Self {
            counts: [0; 65536],
            coarse: [0; 256],
            total: 0,
            median: 0,
            below: 0,
        }
    }

    #[inline(always)]
    fn add(&mut self, val: u16) {
        self.counts[val as usize] += 1;
        self.coarse[(val >> 8) as usize] += 1;
        self.total += 1;
        if val < self.median {
            self.below += 1;
        }
        //  self.rebalance();
    }

    #[inline(always)]
    fn remove(&mut self, val: u16) {
        self.counts[val as usize] -= 1;
        self.coarse[(val >> 8) as usize] -= 1;
        self.total -= 1;
        if val < self.median {
            self.below -= 1;
        }
        //self.rebalance();
    }

    #[inline(always)]
    fn rebalance(&mut self) {
        let target = self.total / 2;

        if self.below > target {
            // Walk DOWN — fine grained only, short distance in practice
            loop {
                if self.median == 0 || self.below <= target {
                    break;
                }
                self.median -= 1;
                self.below -= self.counts[self.median as usize];
            }
        } else {
            // Walk UP with coarse skipping
            loop {
                let at_median = self.counts[self.median as usize];
                if self.below + at_median > target {
                    break; // target rank is inside this fine bucket
                }
                if self.median == 65535 {
                    break;
                }

                // Coarse skip: if we're at the start of a coarse bin (256-wide)
                // and the entire bin lies below target, skip all 256 at once
                let fine_idx = self.median as usize & 0xFF;
                if fine_idx == 0 {
                    let coarse_idx = self.median as usize >> 8;
                    let bin_sum = self.coarse[coarse_idx];
                    if self.below + bin_sum <= target {
                        self.below += bin_sum;
                        self.median = ((coarse_idx + 1) * 256).min(65535) as u16;
                        continue;
                    }
                }

                // Fine step
                self.below += at_median;
                self.median += 1;
            }
        }
    }

    #[inline(always)]
    fn find_median(&mut self) -> u16 {
        self.rebalance();
        self.median
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
        let kernel_h = 2 * radius + 1;
        let mut row_offsets = vec![0usize; kernel_h];

        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;

            // Precompute row offsets for this row's kernel window
            for (i, dy) in (-r_isize..=r_isize).enumerate() {
                let real_y = (global_y.cast_signed() + dy).clamp(0, h_isize - 1) as usize;
                row_offsets[i] = real_y * width;
            }

            // Initialize histogram for x = 0
            let mut hist = Box::new(HistogramU16::new());

            for &row_off in &row_offsets {
                for dx in -r_isize..=r_isize {
                    let real_x = dx.clamp(0, w_isize - 1) as usize;
                    hist.add(src[row_off + real_x]);
                }
            }
            dest[local_y * width] = hist.find_median();

            // Slide right
            for x in 1..width {
                let drop_x = (x.cast_signed() - r_isize - 1).clamp(0, w_isize - 1) as usize;
                let add_x = (x.cast_signed() + r_isize).clamp(0, w_isize - 1) as usize;

                for &row_off in &row_offsets {
                    hist.remove(src[row_off + drop_x]);
                    hist.add(src[row_off + add_x]);
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

        MedianBlur::new(2).execute(&mut img_u8).unwrap();
        MedianBlur::new(2).execute(&mut img_u16).unwrap();

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
