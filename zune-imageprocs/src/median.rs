/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fmt::Debug;

//use crate::spatial::spatial_mut;

pub fn find_median<T: Copy + Ord>(array: &mut [T]) -> T {
    array.sort_unstable();
    let middle = array.len() / 2;

    array[middle]
}

/// Median returns a new image in which each pixel is the median of its neighbors.
/// The parameter radius corresponds to the radius of the neighbor area to be searched,
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
pub fn median<T: Copy + Ord + Default + Debug>(
    _in_channel: &[T], _out_channel: &mut [T], _radius: usize, _width: usize, _height: usize
) {
    //spatial_mut(in_channel, out_channel, radius, width, height, find_median);
}
