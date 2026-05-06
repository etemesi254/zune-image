/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Simple spatial operations implemented for images
use std::fmt::Debug;
use std::ops::{Add, Div, Sub};


use crate::traits::NumOps;

/// Spatial operations implemented for images
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
pub enum SpatialOperations {
    /// (max-min)/(max+min)
    Contrast,
    /// max
    Maximum,
    /// max-min
    Gradient,
    /// min
    Minimum,
    /// sum(pix)/len
    Mean
}

impl SpatialOperations {
    pub fn from_string_result(input: &str) -> Result<Self, String> {
        match input
        {
            "contrast" => Ok(Self::Contrast),
            "maximum" | "max" => Ok(Self::Maximum),
            "gradient" => Ok(Self::Gradient),
            "minimum" | "min" => Ok(Self::Minimum),
            "mean" | "avg" => Ok(Self::Mean),
            _ => Err(
                "Unknown statistic type,accepted values are contrast,(maximum|max),gradient,(minimum|min),mean"
                    .to_string()
            )
        }
    }
}


fn find_min<T: PartialOrd + Copy + NumOps<T>>(data: &[T]) -> T {
    let mut minimum = T::max_val();
    for datum in data {
        if *datum < minimum {
            minimum = *datum;
        }
    }
    minimum
}

fn find_max<T: PartialOrd + Copy + NumOps<T>>(data: &[T]) -> T {
    let mut maximum = T::min_val();
    for datum in data {
        if *datum > maximum {
            maximum = *datum;
        }
    }
    maximum
}

fn find_gradient<T>(data: &[T]) -> T
where
    T: PartialOrd + Copy + NumOps<T> + Sub<Output = T>,
{
    find_max(data) - find_min(data)
}

fn find_contrast<T>(data: &[T]) -> T
where
    T: PartialOrd + Copy + NumOps<T> + Sub<Output = T> + Add<Output = T> + Div<Output = T>,
{
    let minimum = find_min(data);
    let maximum = find_max(data);
    let num = maximum - minimum;
    let div = (maximum + minimum).saturating_add(T::one());
    num / div
}

#[allow(clippy::cast_possible_truncation)]
fn find_mean<T>(data: &[T]) -> T
where
    T: Default + Copy + NumOps<T>,
    u32: std::convert::From<T>,
{
    let mut sum = u32::default();
    let len = data.len() as u32;
    for datum in data {
        sum += u32::from(*datum);
    }
    T::from_u32(sum / len)
}

/// Collect the `(2*radius+1)²` neighbourhood around `(cx, cy)` into `buf`,
/// clamping out-of-bounds coordinates to replicate edge pixels.
#[inline(always)]
fn collect_neighbourhood<T: Copy>(
    src: &[T], width: usize, height: usize, cx: usize, cy: usize, radius: usize, buf: &mut [T],
) {
    let diameter = 2 * radius + 1;
    let mut i = 0;
    for ky in 0..diameter {
        let sy = (cy + ky).saturating_sub(radius).min(height - 1);
        for kx in 0..diameter {
            let sx = (cx + kx).saturating_sub(radius).min(width - 1);
            buf[i] = src[sy * width + sx];
            i += 1;
        }
    }
}

pub fn spatial_ops<T>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize, height: usize,
    operations: SpatialOperations,
) where
    T: PartialOrd
    + Default
    + Copy
    + NumOps<T>
    + Sub<Output = T>
    + Add<Output = T>
    + Div<Output = T>
    + Send
    + Sync,
    u32: std::convert::From<T>,
{
    // See original comment: fn pointers beat a match inside the hot loop
    // due to dramatically better cache behaviour.
    let ptr: fn(&[T]) -> T = match operations {
        SpatialOperations::Contrast  => find_contrast::<T>,
        SpatialOperations::Maximum   => find_max::<T>,
        SpatialOperations::Gradient  => find_gradient::<T>,
        SpatialOperations::Minimum   => find_min::<T>,
        SpatialOperations::Mean      => find_mean::<T>,
    };

    let neighbourhood_len = (2 * radius + 1) * (2 * radius + 1);
    let mut buf = vec![T::default(); neighbourhood_len];

    for y in 0..height {
        for x in 0..width {
            collect_neighbourhood(in_channel, width, height, x, y, radius, &mut buf);
            out_channel[y * width + x] = ptr(&buf);
        }
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use crate::spatial_ops::{spatial_ops, SpatialOperations};

    #[bench]
    fn bench_spatial_mean(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let in_vec = vec![255_u16; dimensions];
        let mut out_vec = vec![255_u16; dimensions];

        let radius = 3;

        b.iter(|| {
            spatial_ops(
                &in_vec,
                &mut out_vec,
                radius,
                width,
                height,
                SpatialOperations::Mean
            );
        });
    }

    #[bench]
    fn bench_spatial_min(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let in_vec = vec![255_u16; dimensions];
        let mut out_vec = vec![255_u16; dimensions];

        let radius = 3;

        b.iter(|| {
            spatial_ops(
                &in_vec,
                &mut out_vec,
                radius,
                width,
                height,
                SpatialOperations::Minimum
            );
        });
    }
}
