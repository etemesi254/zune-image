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

use crate::pad::{pad, PadMethod};
use crate::spatial::spatial;
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

fn find_min<T: PartialOrd + Default + Copy + NumOps<T>>(data: &[T]) -> T {
    let mut minimum = T::max_val();

    for datum in data {
        if *datum < minimum {
            minimum = *datum;
        }
    }
    minimum
}

fn find_contrast<
    T: PartialOrd + Default + Copy + NumOps<T> + Sub<Output = T> + Add<Output = T> + Div<Output = T>
>(
    data: &[T]
) -> T {
    let mut minimum = T::max_val();
    let mut maximum = T::min_val();

    for datum in data {
        if *datum < minimum {
            minimum = *datum;
        }
        if *datum > maximum {
            maximum = *datum;
        }
    }
    let num = maximum - minimum;
    let div = (maximum + minimum).saturating_add(T::one()); // do not allow division by zero

    num / div
}

fn find_gradient<
    T: PartialOrd + Default + Copy + NumOps<T> + Sub<Output = T> + Add<Output = T> + Div<Output = T>
>(
    data: &[T]
) -> T {
    let mut minimum = T::max_val();
    let mut maximum = T::min_val();

    for datum in data {
        if *datum < minimum {
            minimum = *datum;
        }
        if *datum > maximum {
            maximum = *datum;
        }
    }

    maximum - minimum
}

#[inline(always)]
fn find_max<T: PartialOrd + Copy + NumOps<T>>(data: &[T]) -> T {
    let mut maximum = T::min_val();

    for datum in data {
        if *datum > maximum {
            maximum = *datum;
        }
    }
    maximum
}

#[allow(clippy::cast_possible_truncation)]
fn find_mean<T>(data: &[T]) -> T
where
    T: Default + Copy + NumOps<T> + Add<Output = T> + Div<Output = T>,
    u32: std::convert::From<T>
{
    //https://godbolt.org/z/6Y8ncehd5
    let mut maximum = u32::default();
    let len = data.len() as u32;

    for datum in data {
        maximum += u32::from(*datum);
    }
    T::from_u32(maximum / len)
}

/// Run spatial operations on a pixel
///
/// # Arguments
///
/// * `in_channel`:  Input channel.
/// * `out_channel`: Output channels
/// * `radius`:  Radius for the spatial function
/// * `width`:  Image width
/// * `height`:  Image height
/// * `operations`:  Enum operation to run
///
///
pub fn spatial_ops<T>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize, height: usize,
    operations: SpatialOperations
) where
    T: PartialOrd
        + Default
        + Copy
        + NumOps<T>
        + Sub<Output = T>
        + Add<Output = T>
        + Div<Output = T>,
    u32: std::convert::From<T>
{
    //pad here
    let padded_input = pad(
        in_channel,
        width,
        height,
        radius,
        radius,
        PadMethod::Replicate
    );

    // Note: It's faster to do it like this,
    // Because of our tied and tested enemy called cache misses
    //
    // i.e using fn pointers
    //
    //   55,526,220,319   L1-dcache-loads:u         #    3.601 G/sec                    (75.02%)
    //   746,710,874      L1-dcache-load-misses:u   #    1.34% of all L1-dcache accesses  (75.03%)
    //
    // Manual code for each statistic:
    //
    //   40,616,989,582   L1-dcache-loads:u         #    1.451 G/sec                    (75.03%)
    //   103,089,305      L1-dcache-load-misses:u   #    0.25% of all L1-dcache accesses  (75.01%)
    //
    //
    // Fn pointers have it 2x faster , yea tell me that we understand computers.
    let ptr = match operations {
        SpatialOperations::Contrast => find_contrast::<T>,
        SpatialOperations::Maximum => find_max::<T>,
        SpatialOperations::Gradient => find_gradient::<T>,
        SpatialOperations::Minimum => find_min::<T>,
        SpatialOperations::Mean => find_mean::<T>
    };

    spatial(&padded_input, out_channel, radius, width, height, ptr);
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
