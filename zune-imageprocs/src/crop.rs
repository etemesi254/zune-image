/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Crop a single channel
//!
//!
//!  # Algorithm
//!
//! We can take cropping as a view into a sub-image
//! which means
//!
//! ```text
//!    width ──────────────────────────────►
//! │ ┌─────────────────────────────────────┐
//! │ │                                     │
//! │ │                                     │
//! │ │   (x,y)     out width               │
//! │ │     ┌────────────────────┐          │
//! │ │   o │                    │          │
//! │ │   u │                    │          │
//! │ │   t │                    │          │
//! │ │     │  CROPPED IMAGE     │          │
//! │ │   h │                    │          │
//! │ │   e │                    │          │
//! │ │   i │                    │          │
//! │ │   g │                    │          │
//! │ │   h └────────────────────┘          │
//! │ │   t                                 │
//! │ │                                     │
//! ▼ │                                     │
//!   └─────────────────────────────────────┘
//! ```
//! So a crop is essentially a weird memory copy starting from
//! (x,y) like a small sub copy !!!
//! That's what we essentialy implement here
//!
//! ## Specific implementation
//!
//! So because we need to skip from 0--y we can use iter.skip(y) to point at y.
//!
//! Since every iterator is moving a single line per height, we only iterate per
//! out_height number of times, so we can achieve this with a `take` iterators.
//! Rust iterators are fun!!
//!
//!

/// Crop an image channel
///
/// # Arguments
///
/// * `in_image`:   Input image/image channel
/// * `in_width`:   Input width
/// * `out_image`:  Output image/image channel
/// * `out_width`:  Output width
/// * `out_height`: Output height
/// * `x`:  x offset from start(width)
/// * `y`:  y offset from start (height)
///
/// returns: Nothing.
///
/// `out_image` will contain cropped image
///
/// # Notes
/// - If you are cropping an interleaved image using raw bytes,
/// `in_width` is (width*components)
///
/// - If `out_image` is smaller than expected, bottom output will be truncated
///
/// # Panics
/// - If `in_width` > `out_width`
pub fn crop<T: Copy>(
    in_image: &[T], in_width: usize, out_image: &mut [T], out_width: usize, out_height: usize,
    x: usize, y: usize
)
{
    if in_width == 0 || out_width == 0
    {
        // these generate panic paths for chunks_exact so just eliminate them
        return;
    }

    assert!(in_width <= out_width);

    for (single_in_width, single_out_width) in in_image
        .chunks_exact(in_width)
        .skip(y)
        .take(out_height)
        .zip(out_image.chunks_exact_mut(out_width))
    {
        single_out_width.copy_from_slice(&single_in_width[x..x + out_width]);
    }
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    use crate::crop::crop;

    #[bench]
    fn crop_bench(b: &mut test::Bencher)
    {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let c1 = vec![0_u16; dimensions];
        let mut c2 = vec![0_u16; dimensions / 4];

        b.iter(|| {
            crop(&c1, width, &mut c2, width / 2, height / 2, 0, 0);
        });
    }
}
