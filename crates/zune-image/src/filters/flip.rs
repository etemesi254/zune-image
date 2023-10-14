/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_imageprocs::flip::{flip, vertical_flip};

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Creates a vertical mirror image by reflecting
/// the pixels around the central x-axis.
///
///
/// ```text
///
///old image     new image
/// ┌─────────┐   ┌──────────┐
/// │a b c d e│   │j i h g f │
/// │f g h i j│   │e d c b a │
/// └─────────┘   └──────────┘
/// ```
#[derive(Default)]
pub struct Flip;

impl Flip {
    /// Create a new flip operation
    pub fn new() -> Flip {
        Self::default()
    }
}

impl OperationsTrait for Flip {
    fn get_name(&self) -> &'static str {
        "Flip"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.get_depth();

        for inp in image.get_channels_mut(false) {
            match depth.bit_type() {
                BitType::U8 => {
                    flip(inp.reinterpret_as_mut::<u8>()?);
                }
                BitType::U16 => {
                    flip(inp.reinterpret_as_mut::<u16>()?);
                }
                BitType::F32 => {
                    flip(inp.reinterpret_as_mut::<f32>()?);
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

/// Flip the image vertically,( rotate image by 180 degrees)
///
/// ```text
///
///old image     new image
/// ┌─────────┐   ┌──────────┐
/// │a b c d e│   │f g h i j │
/// │f g h i j│   │a b c d e │
/// └─────────┘   └──────────┘
/// ```
///
#[derive(Default)]
pub struct VerticalFlip;

impl VerticalFlip {
    /// Create a new VerticalFlip operation
    pub fn new() -> VerticalFlip {
        Self::default()
    }
}

impl OperationsTrait for VerticalFlip {
    fn get_name(&self) -> &'static str {
        "Vertical Flip"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.get_depth();
        let width = image.get_dimensions().0;

        for inp in image.get_channels_mut(false) {
            match depth.bit_type() {
                BitType::U8 => {
                    vertical_flip(inp.reinterpret_as_mut::<u8>()?, width);
                }
                BitType::U16 => {
                    vertical_flip(inp.reinterpret_as_mut::<u16>()?, width);
                }
                BitType::F32 => {
                    vertical_flip(inp.reinterpret_as_mut::<f32>()?, width);
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
