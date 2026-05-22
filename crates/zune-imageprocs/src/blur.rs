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

use crate::traits::NumOps;
use crate::transpose::Transpose;
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionMut;
use zune_image::traits::{OperationColorValues, OperationsTrait};

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
        let depth = image.depth();
        trace!("Running gaussian blur using parallel regions");

        let radii = create_box_gauss(self.sigma);

        let box_blur_type = BlurType::FastGaussian { sigma: self.sigma };

        match box_blur_type {
            BlurType::FastGaussian { sigma } => {
                impl_fast_gaussian_blur(sigma, image)?;
            }
            _ => todo!(),
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

// --- HORIZONTAL ADAPTERS (IN-PLACE) ---

fn horizontal_blur_region_box_blur<T: NumOps<T> + Default + Copy + Clone>(
    region: &mut PlanarRegionMut<'_, T>, radii: &[usize; 3],
) where
    u32: std::convert::From<T>,
{
    const CHUNK_SIZE: usize = 4;
    let width = region.width;

    // Allocate double-buffered scratch space for 4 rows
    let mut scratch_1 = vec![T::default(); width * CHUNK_SIZE];
    let mut scratch_2 = vec![T::default(); width * CHUNK_SIZE];

    for channel in region.channels.iter_mut() {
        let mut chunk_iter = channel.chunks_exact_mut(width * CHUNK_SIZE);

        for chunk in chunk_iter.by_ref() {
            let width_dims: [Range<usize>; CHUNK_SIZE] =
                core::array::from_fn(|x| width * x..(width * (x + 1)));

            let mut rows = chunk.get_disjoint_mut(width_dims.clone()).unwrap();

            let mut s1 = scratch_1.get_disjoint_mut(width_dims.clone()).unwrap();

            let mut s2 = scratch_2.get_disjoint_mut(width_dims.clone()).unwrap();

            crate::box_blur::box_blur_inner_nx(&rows, &mut s1, width, radii[0]);
            crate::box_blur::box_blur_inner_nx(&s1, &mut s2, width, radii[1]);
            crate::box_blur::box_blur_inner_nx(&s2, &mut rows, width, radii[2]);
        }

        // Clean up any remaining 1 to 3 rows sequentially using the 1D blur
        for row in chunk_iter.into_remainder().chunks_exact_mut(width) {
            let s1 = &mut scratch_1[0..width];
            let s2 = &mut scratch_2[0..width];

            crate::box_blur::box_blur_inner(row, s1, width, radii[0]);
            crate::box_blur::box_blur_inner(s1, s2, width, radii[1]);
            crate::box_blur::box_blur_inner(s2, row, width, radii[2]);
        }
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

use crate::blur::fast_gaussian_blur::impl_fast_gaussian_blur;
use std::convert::TryFrom;
use std::ops::Range;
