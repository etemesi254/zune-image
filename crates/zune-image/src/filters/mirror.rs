/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Mirror image filter
use zune_core::bit_depth::BitType;
use zune_imageprocs::mirror::mirror;
pub use zune_imageprocs::mirror::MirrorMode;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Rearrange the pixels along a certain axis.
///
/// To see the effect of this
/// see the image [mirror-modes](zune_imageprocs::mirror::MirrorMode) documentation
/// for each used mode
pub struct Mirror {
    mode: MirrorMode
}

impl Mirror {
    /// Create a new mirror filter
    pub fn new(mode: MirrorMode) -> Mirror {
        Self { mode }
    }
}

impl OperationsTrait for Mirror {
    fn get_name(&self) -> &'static str {
        "Mirror"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.get_dimensions();
        let depth = image.get_depth();

        for channel in image.get_channels_mut(false) {
            match depth.bit_type() {
                BitType::U8 => {
                    mirror(
                        channel.reinterpret_as_mut::<u8>()?,
                        width,
                        height,
                        self.mode
                    );
                }

                BitType::U16 => {
                    mirror(
                        channel.reinterpret_as_mut::<u16>()?,
                        width,
                        height,
                        self.mode
                    );
                }
                BitType::F32 => {
                    mirror(
                        channel.reinterpret_as_mut::<f32>()?,
                        width,
                        height,
                        self.mode
                    );
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
