/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Resize operation
use zune_core::bit_depth::BitType;
use zune_imageprocs::resize::resize;
pub use zune_imageprocs::resize::ResizeMethod;

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

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
    pub fn new(new_width: usize, new_height: usize, method: ResizeMethod) -> Resize {
        Resize {
            new_height,
            new_width,
            method
        }
    }
}

impl OperationsTrait for Resize {
    fn get_name(&self) -> &'static str {
        "Resize"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (old_w, old_h) = image.get_dimensions();
        let depth = image.get_depth().bit_type();

        let new_length = self.new_width * self.new_height * image.get_depth().size_of();

        match depth {
            BitType::U8 => {
                for old_channel in image.get_channels_mut(false) {
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
                for old_channel in image.get_channels_mut(true) {
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
                for old_channel in image.get_channels_mut(true) {
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
            d => {
                return Err(ImageErrors::ImageOperationNotImplemented(
                    self.get_name(),
                    d
                ))
            }
        }
        image.set_dimensions(self.new_width, self.new_height);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
