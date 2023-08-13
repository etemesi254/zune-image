/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_imageprocs::transpose::{transpose_generic, transpose_u16, transpose_u8};

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Transpose an image
///
/// This mirrors the image along the image top left to bottom-right
/// diagonal
///
/// Done by swapping X and Y indices of the array representation
#[derive(Default)]
pub struct Transpose;

impl Transpose {
    pub fn new() -> Transpose {
        Transpose::default()
    }
}

impl OperationsTrait for Transpose {
    fn get_name(&self) -> &'static str {
        "Transpose"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.get_dimensions();
        let out_dim = width * height * image.get_depth().size_of();

        let depth = image.get_depth();

        for channel in image.get_channels_mut(false) {
            let mut out_channel = Channel::new_with_bit_type(out_dim, depth.bit_type());

            match depth.bit_type() {
                BitType::U8 => {
                    transpose_u8(
                        channel.reinterpret_as::<u8>().unwrap(),
                        out_channel.reinterpret_as_mut::<u8>().unwrap(),
                        width,
                        height
                    );
                    *channel = out_channel;
                }
                BitType::U16 => {
                    transpose_u16(
                        channel.reinterpret_as::<u16>().unwrap(),
                        out_channel.reinterpret_as_mut::<u16>().unwrap(),
                        width,
                        height
                    );
                    *channel = out_channel;
                }
                BitType::F32 => transpose_generic::<f32>(
                    channel.reinterpret_as().unwrap(),
                    out_channel.reinterpret_as_mut().unwrap(),
                    width,
                    height
                ),
                _ => todo!()
            };
        }

        image.set_dimensions(height, width);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
