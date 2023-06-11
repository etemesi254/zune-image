/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

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
pub fn flip<T: Copy>(in_out_image: &mut [T])
{
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

    for (in_dim, out_dim) in in_img_top.iter_mut().zip(in_img_bottom.iter_mut().rev())
    {
        std::mem::swap(in_dim, out_dim);
    }
}

/// Flip an image on the horizontal axis
///
///
/// ```text
///
///old image     new image
/// ┌─────────┐   ┌──────────┐
/// │a b c d e│   │e d c b a │
/// │f g h i j│   │j i h g f │
/// └─────────┘   └──────────┘
/// ```
///
pub fn horizontal_flip<T: Copy>(channel: &mut [T], width: usize)
{
    for single_stride in channel.chunks_exact_mut(width)
    {
        let (f1, f2) = single_stride.split_at_mut(width / 2);
        for (a, b) in f1.iter_mut().zip(f2.iter_mut().rev())
        {
            core::mem::swap(a, b);
        }
    }
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    use crate::flip::flip;

    #[bench]
    fn flip_scalar(b: &mut test::Bencher)
    {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0_u16; dimensions];

        b.iter(|| {
            flip(&mut c1);
        });
    }
}
