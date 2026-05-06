/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Rearrange the pixels along a certain axis.

use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::utils::execute_on;

/// Supported mirror modes, indicating which half of the image is preserved and reflected.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum MirrorMode {
    /// Preserves the Top (North) half and reflects it onto the Bottom half.
    ///
    /// ```text
    /// Old Image      New Image
    /// ┌─────────┐   ┌─────────┐
    /// │ a b c d │   │ a b c d │
    /// │ e f g h │   │ e f g h │
    /// │ i j k l │   │ e f g h │
    /// │ m n o p │   │ a b c d │
    /// └─────────┘   └─────────┘
    /// ```
    North,

    /// Preserves the Bottom (South) half and reflects it onto the Top half.
    ///
    /// ```text
    /// Old Image      New Image
    /// ┌─────────┐   ┌─────────┐
    /// │ a b c d │   │ m n o p │
    /// │ e f g h │   │ i j k l │
    /// │ i j k l │   │ i j k l │
    /// │ m n o p │   │ m n o p │
    /// └─────────┘   └─────────┘
    /// ```
    South,

    /// Preserves the Right (East) half and reflects it onto the Left half.
    ///
    /// ```text
    /// Old Image      New Image
    /// ┌─────────┐   ┌─────────┐
    /// │ a b c d │   │ d c c d │
    /// │ e f g h │   │ h g g h │
    /// │ i j k l │   │ l k k l │
    /// │ m n o p │   │ p o o p │
    /// └─────────┘   └─────────┘
    /// ```
    East,

    /// Preserves the Left (West) half and reflects it onto the Right half.
    ///
    /// ```text
    /// Old Image      New Image
    /// ┌─────────┐   ┌─────────┐
    /// │ a b c d │   │ a b b a │
    /// │ e f g h │   │ e f f e │
    /// │ i j k l │   │ i j j i │
    /// │ m n o p │   │ m n n m │
    /// └─────────┘   └─────────┘
    /// ```
    West
}

/// Applies a mirror reflection to the image along a specified axis.
///
/// This operation duplicates half of the image's pixels and reflects them over
/// the center line, creating perfect symmetry based on the chosen [`MirrorMode`].
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

        let mirror_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
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
            Ok(())
        };
        execute_on(mirror_fn, image, false)
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
