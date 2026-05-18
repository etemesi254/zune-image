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
    Mean,
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

pub(crate) fn find_min<T: PartialOrd + Copy + NumOps<T>>(data: &[T]) -> T {
    let mut minimum = T::max_val();
    for datum in data {
        if *datum < minimum {
            minimum = *datum;
        }
    }
    minimum
}

pub(crate) fn find_max<T: PartialOrd + Copy + NumOps<T>>(data: &[T]) -> T {
    let mut maximum = T::min_val();
    for datum in data {
        if *datum > maximum {
            maximum = *datum;
        }
    }
    maximum
}

pub(crate) fn find_gradient<T>(data: &[T]) -> T
where
    T: PartialOrd + Copy + NumOps<T> + Sub<Output = T>,
{
    find_max(data) - find_min(data)
}

pub(crate) fn find_contrast<T>(data: &[T]) -> T
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
pub(crate) fn find_mean<T>(data: &[T]) -> T
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
