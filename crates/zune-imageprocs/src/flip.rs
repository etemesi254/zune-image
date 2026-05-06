/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Flip filter: Flip an image by reflecting pixels around the x-axis.
//!
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::utils::execute_on;

/// The direction in which to flip the image.
#[derive(Copy, Clone, Debug)]
pub enum FlipDirection {
    /// Creates a horizontal mirror image by reflecting the pixels around the central vertical (Y) axis.
    ///
    /// ```text
    /// Old Image      New Image
    /// ┌─────────┐   ┌─────────┐
    /// │ a b c d │   │ d c b a │
    /// │ e f g h │   │ h g f e │
    /// └─────────┘   └─────────┘
    /// ```
    Horizontal,

    /// Creates a vertical mirror image by reflecting the pixels around the central horizontal (X) axis.
    ///
    /// ```text
    /// Old Image      New Image
    /// ┌─────────┐   ┌─────────┐
    /// │ a b c d │   │ e f g h │
    /// │ e f g h │   │ a b c d │
    /// └─────────┘   └─────────┘
    /// ```
    Vertical,

    /// Rotates the image by 180 degrees.
    ///
    /// This is equivalent to applying both a Horizontal and Vertical flip
    /// (reflecting across the origin).
    ///
    /// ```text
    /// Old Image      New Image
    /// ┌─────────┐   ┌─────────┐
    /// │ a b c d │   │ h g f e │
    /// │ e f g h │   │ d c b a │
    /// └─────────┘   └─────────┘
    /// ```
    Rotate180,
}

/// Flips or rotates an image geometrically.
pub struct Flip {
    flip_direction: FlipDirection
}


impl Flip {
    /// Create a new flip operation
    #[must_use]
    pub fn new(flip_direction: FlipDirection) -> Flip {
        Self { flip_direction }
    }
}

impl OperationsTrait for Flip {
    fn name(&self) -> &'static str {
        "Flip"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();
        let width = image.dimensions().0;

        let flip_fn = |inp: &mut Channel| -> Result<(), ImageErrors> {
            match self.flip_direction {
                FlipDirection::Horizontal => match depth.bit_type() {
                    BitType::U8 => {
                        flop(inp.reinterpret_as_mut::<u8>()?, width);
                    }
                    BitType::U16 => {
                        flop(inp.reinterpret_as_mut::<u16>()?, width);
                    }
                    BitType::F32 => {
                        flop(inp.reinterpret_as_mut::<f32>()?, width);
                    }
                    d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
                },
                FlipDirection::Vertical => match depth.bit_type() {
                    BitType::U8 => {
                        vertical_flip(inp.reinterpret_as_mut::<u8>()?, width);
                    }
                    BitType::U16 => {
                        vertical_flip(inp.reinterpret_as_mut::<u16>()?, width);
                    }
                    BitType::F32 => {
                        vertical_flip(inp.reinterpret_as_mut::<f32>()?, width);
                    }
                    d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
                },
                FlipDirection::Rotate180 => match depth.bit_type() {
                    BitType::U8 => {
                        flip(inp.reinterpret_as_mut::<u8>()?);
                    }
                    BitType::U16 => {
                        flip(inp.reinterpret_as_mut::<u16>()?);
                    }
                    BitType::F32 => {
                        flip(inp.reinterpret_as_mut::<f32>()?);
                    }
                    d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
                }
            }

            Ok(())
        };

        execute_on(flip_fn, image, false)
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

/// Flip an image
///
/// ```text
///
///old image     new image
/// ┌─────────┐   ┌──────────┐
/// │a b c d e│   │j i h g f │
/// │f g h i j│   │e d c b a │
/// └─────────┘   └──────────┘
/// ```
pub fn flip<T: Copy>(in_out_image: &mut [T]) {
    in_out_image.reverse();
}

/// Flip an image on the vertical axis
///
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
pub fn vertical_flip<T: Copy>(channel: &mut [T], width: usize) {
    let len = channel.len();
    let (top, bottom) = channel.split_at_mut(len / 2);

    for (t, b) in top
        .chunks_exact_mut(width)
        .zip(bottom.rchunks_exact_mut(width))
    {
        // This swaps the chunks in-place, perfectly safely, with zero allocations.
        t.swap_with_slice(b);
    }
}

/// Flop an image
///
///```text
///old image     new image
///┌─────────┐   ┌──────────┐
///│a b c d e│   │e d b c a │
///│f g h i j│   │j i h g f │
///└─────────┘   └──────────┘
///```
///
pub fn flop<T: Copy>(in_out_image: &mut [T], width: usize) {
    assert_eq!(
        in_out_image.len() % width,
        0,
        "Width does not evenly divide image"
    );

    for width_chunks in in_out_image.chunks_exact_mut(width) {
        // Reverses just this specific row in place
        width_chunks.reverse();
    }
}
#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use crate::flip::flip;

    #[bench]
    fn flip_scalar(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0_u16; dimensions];

        b.iter(|| {
            flip(&mut c1);
        });
    }
}
