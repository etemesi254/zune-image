/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Rotate an image
//!
//! # WARNING
//! - This only works  for rotating 180 degrees.
//!
//! It doesn't work for other rotate angles, this will be fixed later
//!
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

pub struct Rotate {
    angle: f32
}

impl Rotate {
    #[must_use]
    pub fn new(angle: f32) -> Rotate {
        Rotate { angle }
    }
}

impl OperationsTrait for Rotate {
    fn name(&self) -> &'static str {
        "Rotate"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let im_type = image.depth().bit_type();

        let (width, _) = image.dimensions();

        for channel in image.channels_mut(false) {
            match im_type {
                BitType::U8 => {
                    if (self.angle - 180.0).abs() < f32::EPSILON {
                        rotate_180::<u8>(channel.reinterpret_as_mut()?, width);
                    }
                }
                BitType::U16 => {
                    if (self.angle - 180.0).abs() < f32::EPSILON {
                        rotate_180::<u16>(channel.reinterpret_as_mut()?, width);
                    }
                }
                BitType::F32 => {
                    if (self.angle - 180.0).abs() < f32::EPSILON {
                        rotate_180::<f32>(channel.reinterpret_as_mut()?, width);
                    }
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            };
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

pub fn rotate<T: Copy>(angle: f32, width: usize, in_image: &[T], out_image: &mut [T]) {
    let angle = angle % 360.0;

    if (angle - 180.0).abs() < f32::EPSILON {
        // copy in image to out image
        out_image.copy_from_slice(in_image);
        rotate_180(out_image, width);
    }
}

/// Rotate an image by 180 degrees in place.
///
/// This method is preferred as it does it in place as opposed
/// to the generic rotate which does it out of place
pub fn rotate_180<T: Copy>(in_out_image: &mut [T], width: usize) {
    // swap bottom row with top row

    // divide array into two
    let half = in_out_image.len() / 2;
    let (top, bottom) = in_out_image.split_at_mut(half);

    for (top_chunk, bottom_chunk) in top
        .chunks_exact_mut(width)
        .zip(bottom.chunks_exact_mut(width).rev())
    {
        for (a, b) in top_chunk.iter_mut().zip(bottom_chunk.iter_mut()) {
            core::mem::swap(a, b);
        }
    }
}

fn _rotate_90(_in_image: &[u8], _out_image: &mut [u8], _width: usize, _height: usize) {
    // a 90 degree rotation is a bit cache unfriendly,
    // since widths become heights, but we can still optimize it
    //                   ┌──────┐
    //┌─────────┐        │ ───► │
    //│ ▲       │        │ 90   │
    //│ │       │        │      │
    //└─┴───────┘        │      │
    //                   └──────┘
    //
    // The lower pixel becomes the top most pixel
    //
    // [1,2,3]    [7,4,1]
    // [4,5,6] -> [8,5,2]
    // [7,8,9]    [9,6,3]
}
