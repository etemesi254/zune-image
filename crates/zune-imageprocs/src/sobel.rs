/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Sobel derivative filter
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::traits::NumOps;

/// Perform a sobel image derivative.
///
/// This operation calculates the gradient of the image,
/// which represents how quickly pixel values change from
/// one point to another in both the horizontal and vertical directions.
/// The magnitude and direction of the gradient can be used to detect edges in an image.
///
/// The matrix for sobel is
///
/// Gx matrix
/// ```text
///   -1, 0, 1,
///   -2, 0, 2,
///   -1, 0, 1
/// ```
/// Gy matrix
/// ```text
/// -1,-2,-1,
///  0, 0, 0,
///  1, 2, 1
/// ```
///
/// The window is a 3x3 window.
#[derive(Default, Copy, Clone)]
pub struct Sobel;

impl Sobel {
    #[must_use]
    pub fn new() -> Sobel {
        Self
    }
}

impl OperationsTrait for Sobel {
    fn name(&self) -> &'static str {
        "Sobel"
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth().bit_type();

        // Pre-allocate the destination buffer
        let mut dest_image = image.clone();

        // Sobel ignores alpha
        let ignore_alpha = true;

        match depth {
            BitType::U8 => {
                image.par_process_regions_out_of_place::<u8, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| {
                        gradient_region_u8(region, &SOBEL_GX_I32, &SOBEL_GY_I32);
                    },
                )?;
            }
            BitType::U16 => {
                image.par_process_regions_out_of_place::<u16, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| {
                        gradient_region(region, &SOBEL_GX_I32, &SOBEL_GY_I32);
                    },
                )?;
            }
            BitType::F32 => {
                image.par_process_regions_out_of_place::<f32, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| {
                        gradient_region(region, &SOBEL_GX_F32, &SOBEL_GY_F32);
                    },
                )?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }

        *image = dest_image;
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

#[rustfmt::skip]
const SOBEL_GX_I32: [i32; 9] = [
    -1, 0, 1,
    -2, 0, 2,
    -1, 0, 1,
];
#[rustfmt::skip]
const SOBEL_GY_I32: [i32; 9] = [
    -1, -2, -1,
     0,  0,  0,
     1,  2,  1,
];
#[rustfmt::skip]
const SOBEL_GX_F32: [f32; 9] = [
    -1.0, 0.0, 1.0,
    -2.0, 0.0, 2.0,
    -1.0, 0.0, 1.0,
];
#[rustfmt::skip]
const SOBEL_GY_F32: [f32; 9] = [
    -1.0, -2.0, -1.0,
     0.0,  0.0,  0.0,
     1.0,  2.0,  1.0,
];
#[inline(always)] // Try to get some calc elided
pub fn gradient_region_u8(region: &mut PlanarRegionOut<'_, u8>, gx: &[i32; 9], gy: &[i32; 9]) {
    let width = region.width;
    if region.src_channels.is_empty() || width == 0 {
        return;
    }

    let global_height = region.src_channels[0].len() / width;
    let max_y = global_height.saturating_sub(1);

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;

            let sy_prev = global_y.saturating_sub(1);
            let sy_curr = global_y.min(max_y);
            let sy_next = (global_y + 1).min(max_y);

            let r0 = &src[sy_prev * width..(sy_prev + 1) * width];
            let r1 = &src[sy_curr * width..(sy_curr + 1) * width];
            let r2 = &src[sy_next * width..(sy_next + 1) * width];

            let dest_row = &mut dest[local_y * width..(local_y + 1) * width];

            let compute_edge = |x: usize| -> u8 {
                let mut sum_x: i32 = 0;
                let mut sum_y: i32 = 0;

                for (ky, row) in [r0, r1, r2].iter().enumerate() {
                    for kx in 0..3 {
                        let sx = if kx == 0 {
                            x.saturating_sub(1)
                        } else if kx == 1 {
                            x
                        } else {
                            (x + 1).min(width - 1)
                        };

                        let px = row[sx] as i32;
                        let k = ky * 3 + kx;

                        sum_x += px * gx[k];
                        sum_y += px * gy[k];
                    }
                }

                // Integer sum of squares
                let mag_sq = sum_x * sum_x + sum_y * sum_y;
                // Single f32 sqrt, then fast saturating cast
                (mag_sq as f32).sqrt().round().min(255.0) as u8
            };

            if width == 1 {
                dest_row[0] = compute_edge(0);
                continue;
            }

            // --- LEFT EDGE ---
            dest_row[0] = compute_edge(0);

            // Create sliding windows of size 3 for each row.
            // We also slice dest_row to match the `1..width - 1` bounds.
            let window_iter = r0.windows(3)
                .zip(r1.windows(3))
                .zip(r2.windows(3))
                .zip(&mut dest_row[1..width - 1]);

            for (((w0, w1), w2), dest) in window_iter {
                // w0, w1, and w2 are slices of exactly length 3.
                let p00 = i32::from(w0[0]);
                let p01 = i32::from(w0[1]);
                let p02 = i32::from(w0[2]);

                let p10 = i32::from(w1[0]);
                let p11 = i32::from(w1[1]);
                let p12 = i32::from(w1[2]);

                let p20 = i32::from(w2[0]);
                let p21 = i32::from(w2[1]);
                let p22 = i32::from(w2[2]);

                let sum_x = p00 * gx[0]
                    + p01 * gx[1]
                    + p02 * gx[2]
                    + p10 * gx[3]
                    + p11 * gx[4]
                    + p12 * gx[5]
                    + p20 * gx[6]
                    + p21 * gx[7]
                    + p22 * gx[8];

                let sum_y = p00 * gy[0]
                    + p01 * gy[1]
                    + p02 * gy[2]
                    + p10 * gy[3]
                    + p11 * gy[4]
                    + p12 * gy[5]
                    + p20 * gy[6]
                    + p21 * gy[7]
                    + p22 * gy[8];

                // Square the integers before casting to float.
                let mag_sq = sum_x * sum_x + sum_y * sum_y;

                // Write directly to the mutable reference of our destination slice
                *dest = (mag_sq as f32).sqrt().round().min(255.0) as u8;
            }

            // --- RIGHT EDGE ---
            dest_row[width - 1] = compute_edge(width - 1);
        }
    }
}
pub fn gradient_region<T, Acc>(region: &mut PlanarRegionOut<'_, T>, gx: &[Acc; 9], gy: &[Acc; 9])
where
    T: NumOps<T> + Copy + Default + Send + Sync,
    Acc: Copy
    + Default
    + std::ops::Add<Output = Acc>
    + std::ops::Mul<Output = Acc>
    + Into<f64>
    + From<T>,
{
    let width = region.width;
    if region.src_channels.is_empty() || width == 0 {
        return;
    }

    let global_height = region.src_channels[0].len() / width;
    let max_y = global_height.saturating_sub(1);

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;

            // 1. Pre-calculate vertically clamped rows ONCE per output row
            let sy_prev = global_y.saturating_sub(1);
            let sy_curr = global_y.min(max_y);
            let sy_next = (global_y + 1).min(max_y);

            // Extract exact slices for the 3 working rows.
            // The compiler knows these have length `width`.
            let r0 = &src[sy_prev * width..(sy_prev + 1) * width];
            let r1 = &src[sy_curr * width..(sy_curr + 1) * width];
            let r2 = &src[sy_next * width..(sy_next + 1) * width];

            // Slice the destination row so we can index it directly by `x`
            let dest_row = &mut dest[local_y * width..(local_y + 1) * width];

            // Helper closure for edge pixels (left and right columns)
            let compute_edge = |x: usize| -> T {
                let mut sum_x = Acc::default();
                let mut sum_y = Acc::default();

                for (ky, row) in [r0, r1, r2].iter().enumerate() {
                    for kx in 0..3 {
                        // Safe horizontal clamping for edges
                        let sx = if kx == 0 {
                            x.saturating_sub(1)
                        } else if kx == 1 {
                            x
                        } else {
                            (x + 1).min(width - 1)
                        };

                        let px = Acc::from(row[sx]);
                        let k = ky * 3 + kx;

                        sum_x = sum_x + px * gx[k];
                        sum_y = sum_y + px * gy[k];
                    }
                }

                let gxf: f64 = sum_x.into();
                let gyf: f64 = sum_y.into();
                let magnitude = (gxf * gxf + gyf * gyf).sqrt();
                T::from_f64(magnitude).zclamp(T::min_val(), T::max_val())
            };

            // Handle 1-pixel wide images edge-case gracefully
            if width == 1 {
                dest_row[0] = compute_edge(0);
                continue;
            }

            // --- 2. LEFT EDGE ---
            dest_row[0] = compute_edge(0);

            // --- 3. INTERIOR (Unrolled & Zero Bounds Checks) ---
            // Because x goes from 1 to width-2, x-1..x+1 is guaranteed to
            // be within 0..width, removing inner-loop panics/bounds checks.
            for x in 1..width - 1 {
                let p00 = Acc::from(r0[x - 1]);
                let p01 = Acc::from(r0[x]);
                let p02 = Acc::from(r0[x + 1]);

                let p10 = Acc::from(r1[x - 1]);
                let p11 = Acc::from(r1[x]);
                let p12 = Acc::from(r1[x + 1]);

                let p20 = Acc::from(r2[x - 1]);
                let p21 = Acc::from(r2[x]);
                let p22 = Acc::from(r2[x + 1]);

                let sum_x = p00 * gx[0]
                    + p01 * gx[1]
                    + p02 * gx[2]
                    + p10 * gx[3]
                    + p11 * gx[4]
                    + p12 * gx[5]
                    + p20 * gx[6]
                    + p21 * gx[7]
                    + p22 * gx[8];

                let sum_y = p00 * gy[0]
                    + p01 * gy[1]
                    + p02 * gy[2]
                    + p10 * gy[3]
                    + p11 * gy[4]
                    + p12 * gy[5]
                    + p20 * gy[6]
                    + p21 * gy[7]
                    + p22 * gy[8];

                let gxf: f64 = sum_x.into();
                let gyf: f64 = sum_y.into();
                let magnitude = (gxf * gxf + gyf * gyf).sqrt();

                dest_row[x] = T::from_f64(magnitude).zclamp(T::min_val(), T::max_val());
            }

            // --- 4. RIGHT EDGE ---
            dest_row[width - 1] = compute_edge(width - 1);
        }
    }
}

