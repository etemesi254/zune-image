/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! (BROKEN): Do not use
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::gaussian_blur::{gaussian_blur_u16, gaussian_blur_u8};

/// Perform an unsharpen mask
///
/// This uses the result of a gaussian filter and thresholding to
/// perform the mask calculation
#[derive(Default)]
pub struct Unsharpen {
    sigma:      f32,
    threshold:  u16,
    percentage: u8
}

impl Unsharpen {
    /// Create a new unsharp mask
    ///
    /// # Arguments
    /// - sigma: This value is passed to the gaussian filter,consult [it's documentation](crate::gaussian_blur::GaussianBlur)
    /// on how to use it
    ///
    /// - threshold: If the result of the blur and the initial image is greater than this,  add the difference, otherwise
    /// skip
    ///  - percentage: `threshold*percentage`
    ///
    #[must_use]
    pub fn new(sigma: f32, threshold: u16, percentage: u8) -> Unsharpen {
        Unsharpen {
            sigma,
            threshold,
            percentage
        }
    }
}

impl OperationsTrait for Unsharpen {
    fn name(&self) -> &'static str {
        "Unsharpen"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();

        let depth = image.depth();

        #[cfg(not(feature = "threads"))]
        {
            trace!("Running unsharpen in single threaded mode");

            match depth.bit_type() {
                BitType::U16 => {
                    let mut blur_buffer = vec![0; width * height];
                    let mut blur_scratch = vec![0; width * height];

                    for channel in image.get_channels_mut(true) {
                        unsharpen_u16(
                            channel.reinterpret_as_mut::<u16>()?,
                            &mut blur_buffer,
                            &mut blur_scratch,
                            self.sigma,
                            self.threshold,
                            self.percentage as u16,
                            width,
                            height
                        );
                    }
                }

                BitType::U8 => {
                    let mut blur_buffer = vec![0; width * height];
                    let mut blur_scratch = vec![0; width * height];

                    for channel in image.get_channels_mut(true) {
                        unsharpen_u8(
                            channel.reinterpret_as_mut::<u8>()?,
                            &mut blur_buffer,
                            &mut blur_scratch,
                            self.sigma,
                            self.threshold as u8,
                            self.percentage,
                            width,
                            height
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
            trace!("Running unsharpen in multithreaded mode");
            std::thread::scope(|s| {
                let mut errors = vec![];
                // blur each channel on a separate thread
                for channel in image.channels_mut(true) {
                    let result = s.spawn(|| match depth.bit_type() {
                        BitType::U16 => {
                            let mut blur_buffer = vec![0; width * height];
                            let mut blur_scratch = vec![0; width * height];

                            unsharpen_u16(
                                channel.reinterpret_as_mut::<u16>()?,
                                &mut blur_buffer,
                                &mut blur_scratch,
                                self.sigma,
                                self.threshold,
                                u16::from(self.percentage),
                                width,
                                height
                            );
                            Ok(())
                        }

                        BitType::U8 => {
                            let mut blur_buffer = vec![0; width * height];
                            let mut blur_scratch = vec![0; width * height];

                            unsharpen_u8(
                                channel.reinterpret_as_mut::<u8>()?,
                                &mut blur_buffer,
                                &mut blur_scratch,
                                self.sigma,
                                u8::try_from(self.threshold.clamp(0, 255)).unwrap_or(u8::MAX),
                                self.percentage,
                                width,
                                height
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

///  Sharpen an image
///
///  The underlying algorithm applies a gaussian blur
/// to a copy of the image and compare it with the image,
/// if difference is greater than threshold, we add it to the
/// image
///
/// The formula is
///
/// sharpened = original + (original − blurred);
///
///
/// # Arguments
/// - channel: Incoming pixels, output will be written to the same location
/// - blur_buffer: Temporary location we use to store blur coefficients
/// - blur_scratch_buffer: Temporary location we use during blurring to store blur coefficients
/// - sigma: Radius of blur
/// - threshold: If the difference between original and blurred is greater than this, add the diff to
/// the pixel
///- width,height: Image dimensions.
#[allow(clippy::too_many_arguments)]
pub fn unsharpen_u16(
    channel: &mut [u16], blur_buffer: &mut [u16], blur_scratch_buffer: &mut [u16], sigma: f32,
    threshold: u16, _percentage: u16, width: usize, height: usize
) {
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_u16(blur_buffer, blur_scratch_buffer, width, height, sigma);
    // blur buffer now contains gaussian blurred pixels
    // so iterate replacing them
    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter()) {
        let diff = in_pix.saturating_sub(*blur_pix);
        // pull some branchless tricks to help the optimizer
        // here

        // We conditionally take the added version or whatever we had based on this mask
        //  godbolt link: https://godbolt.org/z/YYnEaPedM

        let threshold_mask = u16::from(diff > threshold).wrapping_sub(1);

        // let diff = (diff * percentage) / 100;

        // if diff > threshold { pix = (diff + pix) } else { pix }
        *in_pix = (in_pix.wrapping_add(diff) & !threshold_mask) | (*in_pix & threshold_mask);
    }
}

///  Sharpen an image
///
///  The underlying algorithm applies a gaussian blur
/// to a copy of the image and compare it with the image,
/// if difference is greater than threshold, we add it to the
/// image
///
/// The formula is
///
/// sharpened = original + (original − blurred);
///
///
/// # Arguments
/// - channel: Incoming pixels, output will be written to the same location
/// - blur_buffer: Temporary location we use to store blur coefficients
/// - blur_scratch_buffer: Temporary location we use during blurring to store blur coefficients
/// - sigma: Radius of blur
/// - threshold: If the difference between original and blurred is greater than this, add the diff to
/// the pixel
///- width,height: Image dimensions.
#[allow(clippy::too_many_arguments)]
pub fn unsharpen_u8(
    channel: &mut [u8], blur_buffer: &mut [u8], blur_scratch_buffer: &mut [u8], sigma: f32,
    threshold: u8, _percentage: u8, width: usize, height: usize
) {
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_u8(blur_buffer, blur_scratch_buffer, width, height, sigma);
    // blur buffer now contains gaussian blurred pixels
    // so iterate replacing them
    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter()) {
        let diff = in_pix.wrapping_sub(*blur_pix);
        // pull some branchless tricks to help the optimizer
        // here

        // We conditionally take the added version or whatever we had based on this mask
        //  godbolt link: https://godbolt.org/z/YYnEaPedM
        let threshold_mask = u8::from(diff > threshold).wrapping_sub(1);

        // if diff > threshold { pix = (diff + pix) } else { pix }
        *in_pix = (in_pix.saturating_add(diff) & !threshold_mask) | (*in_pix & threshold_mask);
    }
}
