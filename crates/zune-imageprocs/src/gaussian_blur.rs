/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! (BROKEN): An implementation of a gaussian-blur.
//!
//! Do not use it, not ready for production
//!
//! This module implements a gaussian blur functions for images
//!
//! The implementation does not give the true gaussian coefficients of the
//! as that is an expensive operation but rather approximates it using a series of
//! box blurs
//!
//! For the math behind it see <https://blog.ivank.net/fastest-gaussian-blur.html>

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::transpose;

#[derive(Default)]
pub struct GaussianBlur {
    sigma: f32
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

    #[allow(clippy::too_many_lines)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();
        let depth = image.depth();

        #[cfg(not(feature = "threads"))]
        {
            trace!("Running gaussian blur in single threaded mode");

            match depth.bit_type() {
                BitType::U8 => {
                    let mut temp = vec![0; width * height];

                    for channel in image.get_channels_mut(false) {
                        gaussian_blur_u8(
                            channel.reinterpret_as_mut::<u8>()?,
                            &mut temp,
                            width,
                            height,
                            self.sigma
                        );
                    }
                }
                BitType::U16 => {
                    let mut temp = vec![0; width * height];

                    for channel in image.get_channels_mut(false) {
                        gaussian_blur_u16(
                            channel.reinterpret_as_mut::<u16>()?,
                            &mut temp,
                            width,
                            height,
                            self.sigma
                        );
                    }
                }
                BitType::F32 => {
                    let mut temp = vec![0.0; width * height];
                    for channel in image.get_channels_mut(false) {
                        gaussian_blur_f32(
                            channel.reinterpret_as_mut()?,
                            &mut temp,
                            width,
                            height,
                            self.sigma
                        );
                    }
                }
                d => {
                    return Err(ImageErrors::ImageOperationNotImplemented(
                        self.get_name(),
                        d
                    ))
                }
            }
        }

        #[cfg(feature = "threads")]
        {
            trace!("Running gaussian blur in multithreaded mode");
            std::thread::scope(|s| {
                let mut errors = vec![];
                // blur each channel on a separate thread
                for channel in image.channels_mut(false) {
                    let result = s.spawn(|| match depth.bit_type() {
                        BitType::U8 => {
                            let mut temp = vec![0; width * height];

                            gaussian_blur_u8(
                                channel.reinterpret_as_mut::<u8>()?,
                                &mut temp,
                                width,
                                height,
                                self.sigma
                            );
                            Ok(())
                        }
                        BitType::U16 => {
                            let mut temp = vec![0; width * height];

                            gaussian_blur_u16(
                                channel.reinterpret_as_mut::<u16>()?,
                                &mut temp,
                                width,
                                height,
                                self.sigma
                            );
                            Ok(())
                        }
                        BitType::F32 => {
                            let mut temp = vec![0.0; width * height];

                            gaussian_blur_f32(
                                channel.reinterpret_as_mut()?,
                                &mut temp,
                                width,
                                height,
                                self.sigma
                            );
                            Ok(())
                        }
                        d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
                    });
                    errors.push(result);
                }
                errors
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
/// Create different box radius for each gaussian kernel function.
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

        // Ideal averaging filter width
        let w_ideal = (12.0 * sigma * sigma / n_float).sqrt() + 1.0;
        let mut wl: i32 = w_ideal.floor() as i32;

        if wl % 2 == 0 {
            wl -= 1;
        };

        let wu = (wl + 2) as usize;

        let wl_float = wl as f32;

        let m_ideal = (12.0 * sigma * sigma
            - n_float * wl_float * wl_float
            - 4.0 * n_float * wl_float
            - 3.0 * n_float)
            / (-4.0 * wl_float - 4.0);

        let m: usize = m_ideal.round() as usize;

        for i in 0..3 {
            if i < m {
                radii[i] = wl as usize;
            } else {
                radii[i] = wu;
            }
        }
    }
    // convert even to odd if there exists such values
    return radii.map(|c| if (c % 2) == 0 { c + 1 } else { c });
}

/// Carry out a gaussian blur on bytes that represent a single image channel
///
///
/// # Arguments
/// - in_out_image: A single image channel, we will store blurred pixels in that same buffer
/// - scratch_space: Buffer used to store intermediate components, dimensions must be equal to
///    `in_out_image`
///  - width,height: Dimensions of the image
///  - sigma: A measure of how much to blur the image by.
pub fn gaussian_blur_u16(
    in_out_image: &mut [u16], scratch_space: &mut [u16], width: usize, height: usize, sigma: f32
) {
    // use the box blur implementation
    let blur_radii = create_box_gauss(sigma);

    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        // carry out horizontal box blur
        // for the first iteration, samples are written to scratch space,
        // so the next iteration, samples should be read from scratch space, as that is our input
        match pos % 2 {
            0 => crate::box_blur::box_blur_inner(in_out_image, scratch_space, width, *blur_radius),
            1 => crate::box_blur::box_blur_inner(scratch_space, in_out_image, width, *blur_radius),
            _ => unreachable!()
        };
    }
    // transpose
    // we do three iterations above, so when that is done, results will always be in
    // scratch_space, so wr transpose writing to in_out_image which is used below
    transpose::transpose_u16(scratch_space, in_out_image, width, height);

    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        // carry out horizontal box blur
        match pos % 2 {
            0 => crate::box_blur::box_blur_inner(in_out_image, scratch_space, height, *blur_radius),
            1 => crate::box_blur::box_blur_inner(scratch_space, in_out_image, height, *blur_radius),
            _ => unreachable!()
        };
    }
    // transpose back
    transpose::transpose_u16(scratch_space, in_out_image, height, width);
}

pub fn gaussian_blur_f32(
    in_out_image: &mut [f32], scratch_space: &mut [f32], width: usize, height: usize, sigma: f32
) {
    // use the box blur implementation
    let blur_radii = create_box_gauss(sigma);

    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        // carry out horizontal box blur
        // for the first iteration, samples are written to scratch space,
        // so the next iteration, samples should be read from scratch space, as that is our input
        match pos % 2 {
            0 => crate::box_blur::box_blur_f32_inner(
                in_out_image,
                scratch_space,
                width,
                *blur_radius
            ),
            1 => crate::box_blur::box_blur_f32_inner(
                scratch_space,
                in_out_image,
                width,
                *blur_radius
            ),
            _ => unreachable!()
        };
    }
    // transpose
    // we do three iterations above, so when that is done, results will always be in
    // scratch_space, so wr transpose writing to in_out_image which is used below
    transpose::transpose_generic(scratch_space, in_out_image, width, height);

    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        // carry out horizontal box blur
        match pos % 2 {
            0 => crate::box_blur::box_blur_f32_inner(
                in_out_image,
                scratch_space,
                height,
                *blur_radius
            ),
            1 => crate::box_blur::box_blur_f32_inner(
                scratch_space,
                in_out_image,
                height,
                *blur_radius
            ),
            _ => unreachable!()
        };
    }
    // transpose back
    transpose::transpose_generic(scratch_space, in_out_image, height, width);
}

/// Carry out a gaussian blur on bytes that represent a single image channel
///
///
/// # Arguments
/// - in_out_image: A single image channel, we will store blurred pixels in that same buffer
/// - scratch_space: Buffer used to store intermediate components, dimensions must be equal to
///    `in_out_image`
///  - width,height: Dimensions of the image
///  - sigma: A measure of how much to blur the image by.
pub fn gaussian_blur_u8(
    in_out_image: &mut [u8], scratch_space: &mut [u8], width: usize, height: usize, sigma: f32
) {
    // use the box blur implementation
    let blur_radii = create_box_gauss(sigma);

    assert_eq!(blur_radii.len(), 3, "Update transpose operations");

    // An optimization applied here was applied from Fabian's
    // fast blurs (https://fgiesen.wordpress.com/2012/08/01/fast-blurs-2/)
    //
    // I.e instead of the code reading

    // for (int pass=0; pass < num_passes; pass++) {
    //   for (int y=0; y < height; y++) {
    //     blur_scanline(y, radius);
    //   }
    // }
    //
    //  It is
    //
    // for (int y=0; y < height; y++) {
    //   for (int pass=0; pass < num_passes; pass++) {
    //     blur_scanline(y, radius);
    //   }
    // }
    //
    // The latter allows us to delay transposition i.e instead
    // of doing blur->transpose->blur->transpose->blur->transpose...
    // we do, blur->blur->blur...->transpose->blur->blur->blur..->transpose
    //
    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        // carry out horizontal box blur
        // for the first iteration, samples are written to scratch space,
        // so the next iteration, samples should be read from scratch space, as that is our input
        match pos % 2 {
            0 => crate::box_blur::box_blur_inner(in_out_image, scratch_space, width, *blur_radius),
            1 => crate::box_blur::box_blur_inner(scratch_space, in_out_image, width, *blur_radius),
            _ => unreachable!()
        };
    }
    // transpose
    // we do three iterations above, so when that is done, results will always be in
    // scratch_space, so wr transpose writing to in_out_image which is used below
    transpose::transpose_u8(scratch_space, in_out_image, width, height);

    for (pos, blur_radius) in blur_radii.iter().enumerate() {
        // carry out horizontal box blur
        match pos % 2 {
            0 => crate::box_blur::box_blur_inner(in_out_image, scratch_space, height, *blur_radius),
            1 => crate::box_blur::box_blur_inner(scratch_space, in_out_image, height, *blur_radius),
            _ => unreachable!()
        };
    }
    // transpose back
    transpose::transpose_u8(scratch_space, in_out_image, height, width);
}
