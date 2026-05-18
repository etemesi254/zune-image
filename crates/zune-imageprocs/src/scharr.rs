/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Scharr derivative filter
use crate::sobel::gradient_region;
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

/// Perform a scharr image derivative.
///
/// This operation calculates the gradient of the image,
/// which represents how quickly pixel values change from
/// one point to another in both the horizontal and vertical directions.
/// The magnitude and direction of the gradient can be used to detect edges in an image.
///
/// The matrix for scharr is
///
/// Gx matrix
/// ```text
///   -3, 0,  3,
///  -10, 0, 10,
///   -3, 0,  3
/// ```
/// Gy matrix
/// ```text
/// -3,-10,-3,
///  0,  0, 0,
///  3, 10, 3
/// ```
///
/// The window is a 3x3 window.
#[derive(Default, Copy, Clone)]
pub struct Scharr;

impl Scharr {
    /// Create a new scharr filter
    #[must_use]
    pub fn new() -> Scharr {
        Self
    }
}

impl OperationsTrait for Scharr {
    fn name(&self) -> &'static str {
        "Scharr"
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
                        gradient_region(region, &SCHARR_GX_I32, &SCHARR_GY_I32);
                    },
                )?;
            }
            BitType::U16 => {
                image.par_process_regions_out_of_place::<u16, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| {
                        gradient_region(region, &SCHARR_GX_I32, &SCHARR_GY_I32);
                    },
                )?;
            }
            BitType::F32 => {
                image.par_process_regions_out_of_place::<f32, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| {
                        gradient_region(region, &SCHARR_GX_F32, &SCHARR_GY_F32);
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
const SCHARR_GX_I32: [i32; 9] = [
     -3, 0,  3,
    -10, 0, 10,
     -3, 0,  3,
];
#[rustfmt::skip]
const SCHARR_GY_I32: [i32; 9] = [
    -3, -10, -3,
     0,   0,  0,
     3,  10,  3,
];
#[rustfmt::skip]
const SCHARR_GX_F32: [f32; 9] = [
     -3.0, 0.0,  3.0,
    -10.0, 0.0, 10.0,
     -3.0, 0.0,  3.0,
];
#[rustfmt::skip]
const SCHARR_GY_F32: [f32; 9] = [
    -3.0, -10.0, -3.0,
     0.0,   0.0,  0.0,
     3.0,  10.0,  3.0,
];