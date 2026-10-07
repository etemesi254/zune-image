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

use std::ops::Range;
use crate::traits::NumOps;
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionMut;
use zune_image::traits::{OperationColorValues, OperationsTrait};
use crate::blur::fast_gaussian_blur::impl_fast_gaussian_blur;

mod fast_gaussian_blur;
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlurType {
    FastGaussian { sigma: f32 },
    BoxApprox(f32),
}
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
/// use zune_image::errors::ImageErrors;
/// use zune_imageprocs::blur::Blur;
///
/// let mut img = Image::fill(255_u8, ColorSpace::RGB, 100, 100);
///
/// // Apply a Gaussian blur with a sigma of 5.0
/// let blur = Blur::new(5.0);
/// blur.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Default)]
pub struct Blur {
    sigma: f32,
}

impl Blur {
    /// Create a new gaussian blur filter
    ///
    /// # Arguments
    /// - sigma: How much to blur by.
    #[must_use]
    pub fn new(sigma: f32) -> Blur {
        Blur { sigma }
    }
}

impl OperationsTrait for Blur {
    fn name(&self) -> &'static str {
        "Gaussian blur"
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        trace!("Running gaussian blur using parallel regions");


        let box_blur_type = BlurType::FastGaussian { sigma: self.sigma };

        match box_blur_type {
            BlurType::FastGaussian { sigma } => {
                impl_fast_gaussian_blur(sigma, image)?;
            }
            BlurType::BoxApprox(_) => todo!(),
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

fn horizontal_blur_region_f32(region: &mut PlanarRegionMut<'_, f32>, radii: &[usize; 3]) {
    let width = region.width;
    let mut scratch_row = vec![0.0f32; width];

    for channel in region.channels.iter_mut() {
        for row in channel.chunks_exact_mut(width) {
            crate::box_blur::box_blur_f32_inner(row, &mut scratch_row, width, radii[0]);
            crate::box_blur::box_blur_f32_inner(&scratch_row, row, width, radii[1]);
            crate::box_blur::box_blur_f32_inner(row, &mut scratch_row, width, radii[2]);
            row.copy_from_slice(&scratch_row);
        }
    }
}

