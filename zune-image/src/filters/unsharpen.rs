/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Unsharpen filer
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_imageprocs::unsharpen::{unsharpen_u16, unsharpen_u8};

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

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
    /// - sigma: This value is passed to the gaussian filter,constult [it's documentation](crate::filters::gaussian_blur::GaussianBlur)
    /// on how to use it
    ///
    /// - threshold: If the result of the blur and the initial image is greater than this,  add the difference, otherwise
    /// skip
    ///  - percentage: `threshold*percentage`
    ///
    pub fn new(sigma: f32, threshold: u16, percentage: u8) -> Unsharpen {
        Unsharpen {
            sigma,
            threshold,
            percentage
        }
    }
}

impl OperationsTrait for Unsharpen {
    fn get_name(&self) -> &'static str {
        "Unsharpen"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.get_dimensions();

        let depth = image.get_depth();

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
                for channel in image.get_channels_mut(true) {
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
                                self.percentage as u16,
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
                                self.threshold as u8,
                                self.percentage,
                                width,
                                height
                            );
                            Ok(())
                        }
                        d => {
                            return Err(ImageErrors::ImageOperationNotImplemented(
                                self.get_name(),
                                d
                            ))
                        }
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
