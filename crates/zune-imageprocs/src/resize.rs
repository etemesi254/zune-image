/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! (BROKEN): Resize an image to a new width and height
//!
//! (BROKEN): Do not use, **IT DOESN'T WORK**
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;

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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (old_w, old_h) = image.dimensions();
        let depth = image.depth().bit_type();

        let new_length = self.new_width * self.new_height * image.depth().size_of();

        match depth {
            BitType::U8 => {
                for old_channel in image.channels_mut(false) {
                    let mut new_channel = Channel::new_with_bit_type(new_length, depth);

                    resize::<u8>(
                        old_channel.reinterpret_as().unwrap(),
                        new_channel.reinterpret_as_mut().unwrap(),
                        self.method,
                        old_w,
                        old_h,
                        self.new_width,
                        self.new_height
                    );
                    *old_channel = new_channel;
                }
            }
            BitType::U16 => {
                for old_channel in image.channels_mut(true) {
                    let mut new_channel = Channel::new_with_bit_type(new_length, depth);

                    resize::<u16>(
                        old_channel.reinterpret_as().unwrap(),
                        new_channel.reinterpret_as_mut().unwrap(),
                        self.method,
                        old_w,
                        old_h,
                        self.new_width,
                        self.new_height
                    );
                    *old_channel = new_channel;
                }
            }
            BitType::F32 => {
                for old_channel in image.channels_mut(true) {
                    let mut new_channel = Channel::new_with_bit_type(new_length, depth);

                    resize::<f32>(
                        old_channel.reinterpret_as().unwrap(),
                        new_channel.reinterpret_as_mut().unwrap(),
                        self.method,
                        old_w,
                        old_h,
                        self.new_width,
                        self.new_height
                    );
                    *old_channel = new_channel;
                }
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
        }

        image.set_dimensions(self.new_width, self.new_height);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
mod bilinear;

#[derive(Copy, Clone, Debug)]
pub enum ResizeMethod {
    Bilinear
}

/// Resize an image to new dimensions
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
    f64: std::convert::From<T>
{
    match method {
        ResizeMethod::Bilinear => {
            bilinear::bilinear_impl(
                in_image, out_image, in_width, in_height, out_width, out_height
            );
        }
    }
}
