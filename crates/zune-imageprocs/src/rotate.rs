/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

pub fn rotate<T: Copy>(angle: f32, width: usize, in_image: &[T], out_image: &mut [T]) {
    let angle = angle % 360.0;

    if (angle - 180.0).abs() < f32::EPSILON {
        // copy in image to out image
        out_image.copy_from_slice(in_image);
        rotate_180(out_image, width);
    }
}

/// Rotate an image by 180 degrees in place.
///
/// This method is preferred as it does it in place as opposed
/// to the generic rotate which does it out of place
pub fn rotate_180<T: Copy>(in_out_image: &mut [T], width: usize) {
    // swap bottom row with top row

    // divide array into two
    let half = in_out_image.len() / 2;
    let (top, bottom) = in_out_image.split_at_mut(half);

    for (top_chunk, bottom_chunk) in top
        .chunks_exact_mut(width)
        .zip(bottom.chunks_exact_mut(width).rev())
    {
        for (a, b) in top_chunk.iter_mut().zip(bottom_chunk.iter_mut()) {
            core::mem::swap(a, b);
        }
    }
}

fn _rotate_90(_in_image: &[u8], _out_image: &mut [u8], _width: usize, _height: usize) {
    // a 90 degree rotation is a bit cache unfriendly,
    // since widths become heights, but we can still optimize it
    //                   ┌──────┐
    //┌─────────┐        │ ───► │
    //│ ▲       │        │ 90   │
    //│ │       │        │      │
    //└─┴───────┘        │      │
    //                   └──────┘
    //
    // The lower pixel becomes the top most pixel
    //
    // [1,2,3]    [7,4,1]
    // [4,5,6] -> [8,5,2]
    // [7,8,9]    [9,6,3]
}
