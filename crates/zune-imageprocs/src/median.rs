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

use crate::utils::{execute_on};
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
        let (width, height) = image.dimensions();

        if self.radius < 1 {
            return Ok(());
        }
        let depth = image.depth();

        let num_threads = image.operation_options().num_threads_child();
        trace!("Running median filter with {} threads", num_threads);

        let median_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

            match depth.bit_type() {
                BitType::U16 => median_u16(
                    channel.reinterpret_as::<u16>()?,
                    new_channel.reinterpret_as_mut::<u16>()?,
                    self.radius,
                    width,
                    height,
                    num_threads,
                ),
                BitType::U8 => median_u8(
                    channel.reinterpret_as::<u8>()?,
                    new_channel.reinterpret_as_mut::<u8>()?,
                    self.radius,
                    width,
                    height,
                    num_threads,
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }
            *channel = new_channel;
            Ok(())
        };

        execute_on(median_fn, image, true)
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
// ============================================================================
// U8 IMPLEMENTATION
// ============================================================================

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

pub fn median_u8(
    in_channel: &[u8], out_channel: &mut [u8], radius: usize, width: usize, height: usize,
    num_threads: usize,
) {
    let chunk_height = (height + num_threads - 1) / num_threads.max(1);
    let chunk_size = chunk_height * width;

    #[cfg(feature = "threads")]
    let use_threads = num_threads > 1;
    #[cfg(not(feature = "threads"))]
    let use_threads = false;

    if use_threads {
        #[cfg(feature = "threads")]
        std::thread::scope(|s| {
            for (i, out_chunk) in out_channel.chunks_mut(chunk_size).enumerate() {
                let start_y = i * chunk_height;
                s.spawn(move || {
                    for (local_y, out_row) in out_chunk.chunks_mut(width).enumerate() {
                        process_row_u8(
                            in_channel,
                            out_row,
                            start_y + local_y,
                            width,
                            height,
                            radius,
                        );
                    }
                });
            }
        });
    } else {
        for (y, out_row) in out_channel.chunks_mut(width).enumerate() {
            process_row_u8(in_channel, out_row, y, width, height, radius);
        }
    }
}

fn process_row_u8(
    input: &[u8], output: &mut [u8], y: usize, width: usize, height: usize, radius: usize,
) {
    let mut hist = HistogramU8::new();
    let r_isize = radius as isize;
    let w_isize = width as isize;
    let h_isize = height as isize;

    // 1. Initialize the histogram for the very first pixel in the row (x = 0)
    for dy in -r_isize..=r_isize {
        let real_y = (y as isize + dy).clamp(0, h_isize - 1) as usize;
        for dx in -r_isize..=r_isize {
            let real_x = dx.clamp(0, w_isize - 1) as usize;
            hist.add(input[real_y * width + real_x]);
        }
    }
    output[0] = hist.find_median();

    // 2. Slide the window to the right
    for x in 1..width {
        let drop_x = (x as isize - r_isize - 1).clamp(0, w_isize - 1) as usize;
        let add_x = (x as isize + r_isize).clamp(0, w_isize - 1) as usize;

        // Add the new rightmost column, drop the old leftmost column
        for dy in -r_isize..=r_isize {
            let real_y = (y as isize + dy).clamp(0, h_isize - 1) as usize;
            let row_offset = real_y * width;
            hist.remove(input[row_offset + drop_x]);
            hist.add(input[row_offset + add_x]);
        }

        output[x] = hist.find_median();
    }
}

// ============================================================================
// U16 IMPLEMENTATION (Tiered Histogram)
// ============================================================================

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

pub fn median_u16(
    in_channel: &[u16], out_channel: &mut [u16], radius: usize, width: usize, height: usize,
    num_threads: usize,
) {
    let chunk_height = (height + num_threads - 1) / num_threads.max(1);
    let chunk_size = chunk_height * width;

    #[cfg(feature = "threads")]
    let use_threads = num_threads > 1;
    #[cfg(not(feature = "threads"))]
    let use_threads = false;

    if use_threads {
        #[cfg(feature = "threads")]
        std::thread::scope(|s| {
            for (i, out_chunk) in out_channel.chunks_mut(chunk_size).enumerate() {
                let start_y = i * chunk_height;
                s.spawn(move || {
                    for (local_y, out_row) in out_chunk.chunks_mut(width).enumerate() {
                        process_row_u16(
                            in_channel,
                            out_row,
                            start_y + local_y,
                            width,
                            height,
                            radius,
                        );
                    }
                });
            }
        });
    } else {
        for (y, out_row) in out_channel.chunks_mut(width).enumerate() {
            process_row_u16(in_channel, out_row, y, width, height, radius);
        }
    }
}

fn process_row_u16(
    input: &[u16], output: &mut [u16], y: usize, width: usize, height: usize, radius: usize,
) {
    let mut hist = HistogramU16::new();
    let r_isize = radius as isize;
    let w_isize = width as isize;
    let h_isize = height as isize;

    // 1. Initialize the histogram for x = 0
    for dy in -r_isize..=r_isize {
        let real_y = (y as isize + dy).clamp(0, h_isize - 1) as usize;
        for dx in -r_isize..=r_isize {
            let real_x = dx.clamp(0, w_isize - 1) as usize;
            hist.add(input[real_y * width + real_x]);
        }
    }
    output[0] = hist.find_median();

    // 2. Slide the window to the right
    for x in 1..width {
        let drop_x = (x as isize - r_isize - 1).clamp(0, w_isize - 1) as usize;
        let add_x = (x as isize + r_isize).clamp(0, w_isize - 1) as usize;

        for dy in -r_isize..=r_isize {
            let real_y = (y as isize + dy).clamp(0, h_isize - 1) as usize;
            let row_offset = real_y * width;
            hist.remove(input[row_offset + drop_x]);
            hist.add(input[row_offset + add_x]);
        }

        output[x] = hist.find_median();
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

        img_u8.iter_pixels::<u8, _>(|_, _, px| {
            assert_eq!(px[0], 128);
        }).unwrap();

        img_u16.iter_pixels::<u16, _>(|_, _, px| {
            assert_eq!(px[0], 32768);
        }).unwrap();
    }

    #[test]
    fn test_median_removes_outlier() {
        // Classic "Salt and Pepper" noise test
        let width = 5;
        let height = 5;
        let mut input = vec![10_u8; width * height];

        // Inject a massive outlier in the center
        input[2 * width + 2] = 255;

        let mut output = vec![0_u8; width * height];

        // A radius of 1 creates a 3x3 window.
        // The center window will see eight 10s and one 255. The median is 10.
        median_u8(&input, &mut output, 1, width, height, 1);

        // The outlier should be completely erased
        assert_eq!(output[2 * width + 2], 10);

        // Everything else should also remain 10
        assert!(output.iter().all(|&x| x == 10));
    }

    #[test]
    fn test_median_u16_tiered_histogram_correctness() {
        let width = 3;
        let height = 3;
        // A 3x3 gradient
        // 100, 200, 300
        // 400, 500, 600
        // 700, 800, 900
        let input: Vec<u16> = vec![
            100, 200, 300,
            400, 500, 600,
            700, 800, 900
        ];
        let mut output = vec![0_u16; width * height];

        // Radius 1 = 3x3 window.
        median_u16(&input, &mut output, 1, width, height, 1);

        // For the center pixel (1,1), the window is the entire image.
        // The sorted array is [100, 200, 300, 400, 500, 600, 700, 800, 900].
        // The exact middle (median) is 500.
        assert_eq!(output[1 * width + 1], 500);
    }

    #[test]
    fn test_median_edge_clamping() {
        let width = 3;
        let height = 3;
        let input: Vec<u8> = vec![
            10, 20, 30,
            40, 50, 60,
            70, 80, 90
        ];
        let mut output = vec![0_u8; width * height];

        // Radius 1 = 3x3 window.
        median_u8(&input, &mut output, 1, width, height, 1);

        // Top-left corner (0,0).
        // Coordinates clamp to >= 0, so the 3x3 virtual window values are:
        // (0,0) clamped from (-1,-1): 10
        // (0,0) clamped from ( 0,-1): 10
        // (1,0) clamped from ( 1,-1): 20
        // (0,0) clamped from (-1, 0): 10
        // (0,0) actual                : 10
        // (1,0) actual                : 20
        // (0,1) clamped from (-1, 1): 40
        // (0,1) actual                : 40
        // (1,1) actual                : 50
        // Sorted: 10, 10, 10, 10, 20, 20, 40, 40, 50.
        // Median (index 4) = 20.
        assert_eq!(output[0], 20);
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;
    use super::*;

    #[bench]
    fn bench_median_u8_r3(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let radius = 3; // 7x7 window
        let input = vec![128_u8; width * height];
        let mut output = vec![0_u8; width * height];

        b.iter(|| {
            median_u8(&input, &mut output, radius, width, height, 1);
        });
    }

    #[bench]
    fn bench_median_u16_r3(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let radius = 3; // 7x7 window
        let input = vec![32768_u16; width * height];
        let mut output = vec![0_u16; width * height];

        b.iter(|| {
            median_u16(&input, &mut output, radius, width, height, 1);
        });
    }

    #[bench]
    fn bench_median_u8_r15(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let radius = 15; // 31x31 window
        let input = vec![128_u8; width * height];
        let mut output = vec![0_u8; width * height];

        // Because it's O(R), this massive 31x31 window will run remarkably fast!
        b.iter(|| {
            median_u8(&input, &mut output, radius, width, height, 1);
        });
    }
}