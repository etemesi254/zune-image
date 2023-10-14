/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
/// Flop operation.
use zune_core::bit_depth::BitType;
use zune_imageprocs::flop::flop;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Creates a horizontal mirror image by reflecting the pixels around the central y-axis
///```text
///old image     new image
///┌─────────┐   ┌──────────┐
///│a b c d e│   │e d b c a │
///│f g h i j│   │j i h g f │
///└─────────┘   └──────────┘
///```
#[derive(Default)]
pub struct Flop;

impl Flop {
    /// Create a new flop implementation
    pub fn new() -> Flop {
        Self::default()
    }
}

impl OperationsTrait for Flop {
    fn get_name(&self) -> &'static str {
        "Flop"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, _) = image.get_dimensions();
        let depth = image.get_depth();

        for channel in image.get_channels_mut(false) {
            match depth.bit_type() {
                BitType::U8 => {
                    flop(channel.reinterpret_as_mut::<u8>()?, width);
                }
                BitType::U16 => {
                    flop(channel.reinterpret_as_mut::<u16>()?, width);
                }
                BitType::F32 => {
                    flop(channel.reinterpret_as_mut::<f32>()?, width);
                }
                d => {
                    return Err(ImageErrors::ImageOperationNotImplemented(
                        self.get_name(),
                        d
                    ))
                }
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
