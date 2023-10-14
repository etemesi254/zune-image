/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Box blur filter
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_imageprocs::box_blur::{box_blur_f32, box_blur_u16, box_blur_u8};

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Perform a box blur
///
/// Radius is a measure of how many
/// pixels to include in the box blur.
///
/// The greater the radius, the more pronounced the box blur
///
/// This operation is multithreaded capable
#[derive(Default)]
pub struct BoxBlur {
    radius: usize
}

impl BoxBlur {
    /// Create a new blur operation.
    ///
    /// # Arguments
    /// - radius: The radius of the blur, larger the value the more pronounced the blur
    pub fn new(radius: usize) -> BoxBlur {
        BoxBlur { radius }
    }
}

impl OperationsTrait for BoxBlur {
    fn get_name(&self) -> &'static str {
        "Box blur"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.get_dimensions();

        let depth = image.get_depth();

        #[cfg(feature = "threads")]
        {
            trace!("Running box blur in multithreaded mode");
            std::thread::scope(|s| {
                let mut errors = vec![];
                // blur each channel on a separate thread
                for channel in image.get_channels_mut(false) {
                    let result = s.spawn(|| match depth.bit_type() {
                        BitType::U16 => {
                            let mut scratch_space = vec![0; width * height];
                            let data = channel.reinterpret_as_mut::<u16>()?;
                            box_blur_u16(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }
                        BitType::U8 => {
                            let mut scratch_space = vec![0; width * height];
                            let data = channel.reinterpret_as_mut::<u8>()?;
                            box_blur_u8(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }

                        BitType::F32 => {
                            let mut scratch_space = vec![0.0; width * height];
                            let data = channel.reinterpret_as_mut::<f32>()?;
                            box_blur_f32(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }
                        d => return Err(ImageErrors::ImageOperationNotImplemented("box_blur", d))
                    });
                    errors.push(result);
                }
                errors
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running box blur in single threaded mode");

            match depth.bit_type() {
                BitType::U16 => {
                    let mut scratch_space = vec![0; width * height];

                    for channel in image.get_channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<u16>()?;
                        box_blur_u16(data, &mut scratch_space, width, height, self.radius);
                    }
                }
                BitType::U8 => {
                    let mut scratch_space = vec![0; width * height];

                    for channel in image.get_channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<u8>()?;
                        box_blur_u8(data, &mut scratch_space, width, height, self.radius);
                    }
                }

                BitType::F32 => {
                    let mut scratch_space = vec![0.0; width * height];

                    for channel in image.get_channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<f32>()?;
                        box_blur_f32(data, &mut scratch_space, width, height, self.radius);
                    }
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented("box_blur", d))
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
