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
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

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
    #[must_use]
    pub fn new() -> Flip {
        Self
    }
}

impl OperationsTrait for Flip {
    fn name(&self) -> &'static str {
        "Flip"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();

        for inp in image.channels_mut(false) {
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
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
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
    #[must_use]
    pub fn new() -> VerticalFlip {
        Self
    }
}

impl OperationsTrait for VerticalFlip {
    fn name(&self) -> &'static str {
        "Vertical Flip"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();
        let width = image.dimensions().0;

        for inp in image.channels_mut(false) {
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
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
        }

        Ok(())
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
    // NOTE: CAE, this operation became slower after switching to generics
    //
    // The compiler fails to see how we can make it faster
    //
    // Original
    //
    // test flip::benchmarks::flip_scalar   ... bench:      20,777 ns/iter (+/- 655)
    //
    // After
    //
    //test flip::benchmarks::flip_scalar    ... bench:      41,956 ns/iter (+/- 4,189)
    //
    // It's still fast enough so hopefully no one notices
    let length = in_out_image.len() / 2;

    let (in_img_top, in_img_bottom) = in_out_image.split_at_mut(length);

    for (in_dim, out_dim) in in_img_top.iter_mut().zip(in_img_bottom.iter_mut().rev()) {
        std::mem::swap(in_dim, out_dim);
    }
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
pub fn vertical_flip<T: Copy + Default>(channel: &mut [T], width: usize) {
    // Simply split the image in half
    // on one end read from the start to the halfway point
    // on the other end read from the end to the halfway point

    let len = channel.len();

    let (top, bottom) = channel.split_at_mut(len / 2);

    let mut stride = vec![T::default(); width];
    for (t, b) in top
        .chunks_exact_mut(width)
        .zip(bottom.rchunks_exact_mut(width))
    {
        stride.copy_from_slice(t);
        t.copy_from_slice(b);
        b.copy_from_slice(&stride);
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
