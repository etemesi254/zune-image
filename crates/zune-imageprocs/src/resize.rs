/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Resize an image to a new width and height
//!
//!
//! Currently only implements a simple bilinear resizer, future plans are to have
//! more complicated resizers implemented.
//!
//!
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;

mod bicubic;
mod bilinear;

#[derive(Copy, Clone, Debug)]
pub enum ResizeMethod {
    Bilinear //Bicubic
}

/// Resize an image to a new width and height
/// using the resize method specified
#[derive(Copy, Clone)]
pub struct Resize {
    new_width:  usize,
    new_height: usize,
    method:     ResizeMethod
}

impl Resize {
    /// Create a new resize operation
    ///
    /// # Argument
    /// - new_width: The new image width
    /// - new_height: The new image height.
    /// - method: The resize method to use
    #[must_use]
    pub fn new(new_width: usize, new_height: usize, method: ResizeMethod) -> Resize {
        Resize {
            new_width,
            new_height,
            method
        }
    }
}

impl OperationsTrait for Resize {
    fn name(&self) -> &'static str {
        "Resize"
    }

    #[allow(clippy::too_many_lines)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (old_w, old_h) = image.dimensions();
        let depth = image.depth().bit_type();

        let new_length = self.new_width * self.new_height * image.depth().size_of();

        #[cfg(feature = "threads")]
        {
            std::thread::scope(|f| {
                let mut errors = vec![];

                for old_channel in image.channels_mut(false) {
                    let result = f.spawn(|| {
                        let mut new_channel = Channel::new_with_bit_type(new_length, depth);
                        match depth {
                            BitType::U8 => resize::<u8>(
                                old_channel.reinterpret_as()?,
                                new_channel.reinterpret_as_mut()?,
                                self.method,
                                old_w,
                                old_h,
                                self.new_width,
                                self.new_height
                            ),
                            BitType::U16 => resize::<u16>(
                                old_channel.reinterpret_as()?,
                                new_channel.reinterpret_as_mut()?,
                                self.method,
                                old_w,
                                old_h,
                                self.new_width,
                                self.new_height
                            ),

                            BitType::F32 => {
                                resize::<f32>(
                                    old_channel.reinterpret_as()?,
                                    new_channel.reinterpret_as_mut()?,
                                    self.method,
                                    old_w,
                                    old_h,
                                    self.new_width,
                                    self.new_height
                                );
                            }
                            d => return Err(ImageErrors::ImageOperationNotImplemented("resize", d))
                        }
                        *old_channel = new_channel;
                        Ok(())
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
            for old_channel in image.channels_mut(false) {
                let mut new_channel = Channel::new_with_bit_type(new_length, depth);
                match depth {
                    BitType::U8 => resize::<u8>(
                        old_channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        self.method,
                        old_w,
                        old_h,
                        self.new_width,
                        self.new_height
                    ),
                    BitType::U16 => resize::<u16>(
                        old_channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        self.method,
                        old_w,
                        old_h,
                        self.new_width,
                        self.new_height
                    ),

                    BitType::F32 => {
                        resize::<f32>(
                            old_channel.reinterpret_as()?,
                            new_channel.reinterpret_as_mut()?,
                            self.method,
                            old_w,
                            old_h,
                            self.new_width,
                            self.new_height
                        );
                    }
                    d => return Err(ImageErrors::ImageOperationNotImplemented("resize", d))
                }
                *old_channel = new_channel;
            }
        }
        image.set_dimensions(self.new_width, self.new_height);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

/// Return the image resize dimensions that would not cause a distortion
/// taking into consideration the smaller dimension
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn ratio_dimensions_smaller(
    old_w: usize, old_h: usize, new_w: usize, new_h: usize
) -> (usize, usize) {
    let ratio_w = old_w as f64 / new_w as f64;
    let ratio_h = old_h as f64 / new_h as f64;
    let percent = if ratio_h < ratio_w { ratio_w } else { ratio_h };

    let t = (old_w as f64 / percent) as usize;
    let u = (old_h as f64 / percent) as usize;
    (t, u)
}

/// Return the image resize dimensions that would not cause a distortion
/// taking into consideration the larger dimension
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn ratio_dimensions_larger(
    old_w: usize, old_h: usize, new_w: usize, new_h: usize
) -> (usize, usize) {
    let ratio_w = old_w as f64 / new_w as f64;
    let ratio_h = old_h as f64 / new_h as f64;
    let percent = if ratio_h < ratio_w { ratio_w } else { ratio_h };

    let t = (old_w as f64 / percent) as usize;
    let u = (old_h as f64 / percent) as usize;
    (t, u)
}
/// Resize an image **channel** to new dimensions
///
/// # Arguments
/// - in_image: A contiguous slice of a single channel of an image
/// - out_image: Where we will store the new resized pixels
/// - method: The resizing method to use
/// - in_width: `in_image`'s width
/// - in_height:  `in_image`'s height.
/// - out_width: The expected width
/// - out_height: The expected height.
/// # Panics
/// - `in_width*in_height` do not match `in_image.len()`.
/// - `out_width*out_height` do not match `out_image.len()`.
pub fn resize<T>(
    in_image: &[T], out_image: &mut [T], method: ResizeMethod, in_width: usize, in_height: usize,
    out_width: usize, out_height: usize
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
{
    match method {
        ResizeMethod::Bilinear => {
            bilinear::bilinear_impl(
                in_image, out_image, in_width, in_height, out_width, out_height
            );
        } // ResizeMethod::Bicubic => {
          //     bicubic::resize_image_bicubic(
          //         in_image, out_image, in_width, in_height, out_width, out_height
          //     );
          // }
    }
}
