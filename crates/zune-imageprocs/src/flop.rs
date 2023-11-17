/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Flop : Reflect pixels around the central y-axis
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

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
    #[must_use]
    pub fn new() -> Flop {
        Self
    }
}

impl OperationsTrait for Flop {
    fn name(&self) -> &'static str {
        "Flop"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, _) = image.dimensions();
        let depth = image.depth();

        for channel in image.channels_mut(false) {
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
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
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
        let (left_to_right, right_to_left) = width_chunks.split_at_mut(width / 2);

        // iterate and swap
        for (ltr, rtl) in left_to_right.iter_mut().zip(right_to_left.iter_mut().rev()) {
            std::mem::swap(ltr, rtl);
        }
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    #[bench]
    fn flop_scalar(b: &mut test::Bencher) {
        use crate::flop::flop;

        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0_u16; dimensions];

        b.iter(|| {
            flop(&mut c1, width);
        });
    }
}
