/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Rearrange the pixels along a certain axis.

use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

/// Supported mirror modes
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum MirrorMode {
    ///
    /// ```text           
    ///  old image     new image
    ///  ┌─────────┐   ┌──────────┐
    ///  │a b c d e│   │a b c d e │
    ///  │f g h i j│   │a b d d e │
    ///  └─────────┘   └──────────┘
    /// ```
    North,
    ///
    /// ```text           
    ///  old image     new image
    ///  ┌─────────┐   ┌──────────┐
    ///  │a b c d e│   │f g h i j │
    ///  │f g h i j│   │f g h i j │
    ///  └─────────┘   └──────────┘
    /// ```
    South,
    ///
    /// ```text           
    ///  old image     new image
    ///  ┌─────────┐   ┌──────────┐
    ///  │a b c d e│   │a b c b a │
    ///  │f g h i j│   │f g h g f │
    ///  └─────────┘   └──────────┘
    /// ```
    East,
    ///
    /// ```text           
    ///  old image     new image
    ///  ┌─────────┐   ┌──────────┐
    ///  │a b c d e│   │e d c d e │
    ///  │f g h i j│   │j i h i j │
    ///  └─────────┘   └──────────┘
    /// ```
    West
}

/// Rearrange the pixels along a certain axis.
///
/// To see the effect of this
/// see the image [mirror-modes](crate::mirror::MirrorMode) documentation
/// for each used mode
pub struct Mirror {
    mode: MirrorMode
}

impl Mirror {
    /// Create a new mirror filter
    #[must_use]
    pub fn new(mode: MirrorMode) -> Mirror {
        Self { mode }
    }
}

impl OperationsTrait for Mirror {
    fn name(&self) -> &'static str {
        "Mirror"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();
        let depth = image.depth();

        for channel in image.channels_mut(false) {
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
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

/// Mirror an image by duplicating pixels from one edge to the other half
///
/// E.g a mirror along the east direction looks like
///
/// ```text           
///  old image     new image
///  ┌─────────┐   ┌──────────┐
///  │a b c d e│   │a b c b a │
///  │f g h i j│   │f g h g f │
///  └─────────┘   └──────────┘
/// ```
pub fn mirror<T: Copy>(in_pixels: &mut [T], width: usize, height: usize, mode: MirrorMode) {
    if mode == MirrorMode::East || mode == MirrorMode::West {
        for width_stride in in_pixels.chunks_exact_mut(width) {
            // split into 2
            let (left, right) = width_stride.split_at_mut(width / 2);

            if mode == MirrorMode::West {
                // write
                left.iter().zip(right.iter_mut().rev()).for_each(|(l, r)| {
                    *r = *l;
                });
            }
            if mode == MirrorMode::East {
                // write
                left.iter_mut().zip(right.iter().rev()).for_each(|(l, r)| {
                    *l = *r;
                });
            }
        }
    } else if mode == MirrorMode::North || mode == MirrorMode::South {
        // split the image along the halfway axis
        let halfway = width * (height / 2);

        let (top, bottom) = in_pixels.split_at_mut(halfway);

        for (top_width_stride, bottom_width_stride) in top
            .chunks_exact_mut(width)
            .zip(bottom.rchunks_exact_mut(width))
        {
            if mode == MirrorMode::North {
                bottom_width_stride.copy_from_slice(top_width_stride);
            } else if mode == MirrorMode::South {
                top_width_stride.copy_from_slice(bottom_width_stride);
            }
        }
    }
}
