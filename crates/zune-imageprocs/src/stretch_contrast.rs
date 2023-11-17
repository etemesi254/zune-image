/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Strech contrast filter, Linearly stretches the contrast in an image

/// Linearly stretches the contrast in an image in place,
/// sending lower to image minimum and upper to image maximum.
#[derive(Default)]
pub struct StretchContrast {
    lower: f32,
    upper: f32
}

impl StretchContrast {
    /// Create a new stretch contrast filter
    ///
    /// # Arguments
    /// - lower: Lower minimum value for which pixels below this are clamped to the value
    /// - upper: Upper maximum value for which pixels above are clamped to the value
    #[must_use]
    pub fn new(lower: f32, upper: f32) -> StretchContrast {
        StretchContrast { lower, upper }
    }
}

impl OperationsTrait for StretchContrast {
    fn name(&self) -> &'static str {
        "Stretch Contrast"
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();

        for channel in image.channels_mut(true) {
            match depth.bit_type() {
                BitType::U8 => stretch_contrast(
                    channel.reinterpret_as_mut::<u8>()?,
                    self.lower as u8,
                    self.upper as u8,
                    u32::from(depth.max_value())
                )?,
                BitType::U16 => stretch_contrast(
                    channel.reinterpret_as_mut::<u16>()?,
                    self.lower as _,
                    self.upper as _,
                    u32::from(depth.max_value())
                )?,
                BitType::F32 => stretch_contrast_f32(
                    channel.reinterpret_as_mut::<f32>()?,
                    self.lower as _,
                    self.upper as _
                )?,
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
        }
        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
use std::ops::Sub;

use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::mathops::{compute_mod_u32, fastdiv_u32};
use crate::traits::NumOps;

///
/// Linearly stretches the contrast in an image in place,
/// sending lower to image minimum and upper to image maximum.
///
/// # Arguments
///
/// * `image`: Image channel pixels
/// * `lower`:  The lower minimum for which pixels below this value
///  become 0
/// * `upper`:  Upper maximum for which pixels above this value become the maximum
///  value
/// * `maximum`: Maximum value for this pixel type.
///
/// - Modifies array in place
///
pub fn stretch_contrast<T>(
    image: &mut [T], lower: T, upper: T, maximum: u32
) -> Result<(), &'static str>
where
    T: Ord + Sub<Output = T> + NumOps<T> + Copy,
    u32: std::convert::From<T>
{
    if upper < lower {
        return Err("upper must be strictly greater than lower");
    }

    let len = u32::from(upper.saturating_sub(lower)).saturating_add(1);

    // using fast division is faster than the other one
    // (weirdly) vectorized one.
    //
    // image dimensions: 5796 * 3984 RGB
    //  using the vectorizable one: 303 ms
    //  using fastdiv:              142 ms
    //
    // Probably due to better pipelining.
    let mod_len = compute_mod_u32(u64::from(len));

    for pixel in image.iter_mut() {
        if *pixel >= upper {
            *pixel = T::max_val();
        } else if *pixel <= lower {
            *pixel = T::min_val();
        } else {
            let numerator = maximum * u32::from(*pixel - lower);
            let scaled = fastdiv_u32(numerator, mod_len);
            *pixel = T::from_u32(scaled);
        }
    }
    Ok(())
}

pub fn stretch_contrast_f32(image: &mut [f32], lower: f32, upper: f32) -> Result<(), &'static str> {
    if upper < lower {
        return Err("upper must be strictly greater than lower");
    }
    let inv_range = 1. / (upper - lower);
    for pixel in image.iter_mut() {
        if *pixel > upper {
            *pixel = f32::max_val();
        } else if *pixel <= lower {
            *pixel = f32::min_val();
        } else {
            *pixel = (f32::max_val() - *pixel) * inv_range;
        }
    }

    Ok(())
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use nanorand::Rng;

    use crate::stretch_contrast::{stretch_contrast, stretch_contrast_f32};

    #[bench]
    fn bench_stretch_contrast(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let mut in_vec = vec![255_u16; dimensions];
        nanorand::WyRand::new().fill(&mut in_vec);

        b.iter(|| {
            stretch_contrast(&mut in_vec, 3, 10, 65535).unwrap();
        });
    }

    #[bench]
    fn bench_stretch_contrast_f32(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let mut in_vec = vec![0.0; dimensions];
        nanorand::WyRand::new().fill(&mut in_vec);

        b.iter(|| {
            stretch_contrast_f32(&mut in_vec, 0.5, 0.8).unwrap();
        });
    }
}
