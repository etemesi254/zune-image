/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::gaussian_blur::{gaussian_blur_u16, gaussian_blur_u8};

/// Sharpen an image
///
/// This uses the result of a gaussian filter and thresholding to
/// perform the mask calculation
#[derive(Default)]
pub struct Sharpen {
    sigma:      f32,
    threshold:  u16,
    percentage: u8
}

impl Sharpen {
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
    pub fn new(sigma: f32, threshold: u16, percentage: u8) -> Sharpen {
        Sharpen {
            sigma,
            threshold,
            percentage
        }
    }
}

impl OperationsTrait for Sharpen {
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

                    for channel in image.channels_mut(true) {
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

                    for channel in image.channels_mut(true) {
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
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
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

#[allow(clippy::too_many_arguments)]
fn unsharpen_u8(
    channel: &mut [u8], blur_buffer: &mut [u8], blur_scratch_buffer: &mut [u8], sigma: f32,
    threshold: u8, percentage: u8, width: usize, height: usize
) {
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_u8(blur_buffer, blur_scratch_buffer, width, height, sigma);

    let pct = i32::from(percentage);
    let thresh = i32::from(threshold);

    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter()) {
        let orig = i32::from(*in_pix);
        let blurred = i32::from(*blur_pix);

        // Signed difference allows us to lighten OR darken the pixel
        let diff = orig - blurred;

        // Check against threshold using absolute magnitude
        if diff.abs() > thresh {
            // Apply the percentage intensity
            let scaled_diff = (diff * pct) / 100;

            // Add back to original and clamp to valid u8 range
            let new_val = orig + scaled_diff;
            *in_pix = new_val.clamp(0, 255) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn unsharpen_u16(
    channel: &mut [u16], blur_buffer: &mut [u16], blur_scratch_buffer: &mut [u16], sigma: f32,
    threshold: u16, percentage: u16, width: usize, height: usize
) {
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_u16(blur_buffer, blur_scratch_buffer, width, height, sigma);

    let pct = i32::from(percentage);
    let thresh = i32::from(threshold);

    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter()) {
        let orig = i32::from(*in_pix);
        let blurred = i32::from(*blur_pix);

        // Signed difference allows us to lighten OR darken the pixel
        let diff = orig - blurred;

        if diff.abs() > thresh {
            // Apply the percentage intensity
            let scaled_diff = (diff * pct) / 100;

            // Add back to original and clamp to valid u16 range
            let new_val = orig + scaled_diff;
            *in_pix = new_val.clamp(0, 65535) as u16;
        }
    }
}
