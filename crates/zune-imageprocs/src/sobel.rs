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
                        gradient_region(region, &SOBEL_GX_I32, &SOBEL_GY_I32);
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

    // Derive global height safely from the full source slice
    let global_height = region.src_channels[0].len() / width;

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;

            for x in 0..width {
                let mut sum_x = Acc::default();
                let mut sum_y = Acc::default();

                // 3x3 Kernel Window
                for ky in 0..3usize {
                    // Safe global edge clamping
                    let sy = (global_y + ky).saturating_sub(1).min(global_height - 1);
                    let row_offset = sy * width;

                    for kx in 0..3usize {
                        let sx = (x + kx).saturating_sub(1).min(width - 1);

                        let px = Acc::from(src[row_offset + sx]);
                        let k = ky * 3 + kx;

                        sum_x = sum_x + px * gx[k];
                        sum_y = sum_y + px * gy[k];
                    }
                }

                // Compute Magnitude: sqrt(Gx^2 + Gy^2)
                let gxf: f64 = sum_x.into();
                let gyf: f64 = sum_y.into();
                let magnitude = (gxf * gxf + gyf * gyf).sqrt();

                dest[local_y * width + x] =
                    T::from_f64(magnitude).zclamp(T::min_val(), T::max_val());
            }
        }
    }
}

