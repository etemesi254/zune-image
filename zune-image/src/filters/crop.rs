/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
/// Crop operaion
use zune_core::bit_depth::BitType;
use zune_imageprocs::crop::crop;

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Crop out a part of an image  
///
/// This creates a smaller image from a bigger image
pub struct Crop {
    x:      usize,
    y:      usize,
    width:  usize,
    height: usize
}

impl Crop {
    /// Create a new crop operation
    ///
    /// # Arguments
    /// - width: The width of the new cropped out image
    /// - height: The height of the new cropped out image.
    /// -x: How far from the x origin the image should start from
    /// -y: How far from the y origin the image should start from
    ///
    /// Origin is defined as the image top left corner.
    pub fn new(width: usize, height: usize, x: usize, y: usize) -> Crop {
        Crop {
            x,
            y,
            width,
            height
        }
    }
}

impl OperationsTrait for Crop {
    fn get_name(&self) -> &'static str {
        "Crop"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let new_dims = self.width * self.height * image.get_depth().size_of();
        let (old_width, _) = image.get_dimensions();
        let depth = image.get_depth().bit_type();

        for channel in image.get_channels_mut(false) {
            let mut new_vec = Channel::new_with_length_and_type(new_dims, channel.get_type_id());

            // since crop is just bytewise copies, we can use the lowest common denominator for it
            // and it will still work
            match depth {
                BitType::U8 => {
                    crop::<u8>(
                        channel.reinterpret_as()?,
                        old_width,
                        new_vec.reinterpret_as_mut()?,
                        self.width,
                        self.height,
                        self.x,
                        self.y
                    );
                }
                BitType::U16 => {
                    crop::<u16>(
                        channel.reinterpret_as()?,
                        old_width,
                        new_vec.reinterpret_as_mut()?,
                        self.width,
                        self.height,
                        self.x,
                        self.y
                    );
                }
                BitType::F32 => {
                    crop::<f32>(
                        channel.reinterpret_as()?,
                        old_width,
                        new_vec.reinterpret_as_mut()?,
                        self.width,
                        self.height,
                        self.x,
                        self.y
                    );
                }
                d => {
                    return Err(ImageErrors::ImageOperationNotImplemented(
                        self.get_name(),
                        d
                    ))
                }
            }
            *channel = new_vec;
        }

        image.set_dimensions(self.width, self.height);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
