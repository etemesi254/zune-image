use std::cmp::{max, min};
use std::fmt::Debug;
use std::ops::{Add, Div, Sub};

use crate::pad::{pad, PadMethod};
use crate::spatial::spatial;
use crate::traits::NumOps;

#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
pub enum StatisticOperations
{
    Contrast,
    Maximum,
    Gradient,
    Minimum,
    Mean
}

impl StatisticOperations
{
    pub fn from_string_result(input: &str) -> Result<Self, String>
    {
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

fn find_min<T: Ord + Default + Copy + NumOps<T>>(data: &[T]) -> T
{
    let mut minimum = T::max_val();

    for datum in data
    {
        minimum = min(*datum, minimum);
    }
    minimum
}

fn find_contrast<
    T: Ord + Default + Copy + NumOps<T> + Sub<Output = T> + Add<Output = T> + Div<Output = T>
>(
    data: &[T]
) -> T
{
    let mut minimum = T::max_val();
    let mut maximum = T::min_val();

    for datum in data
    {
        minimum = min(*datum, minimum);
        maximum = max(*datum, maximum);
    }
    let num = maximum - minimum;
    let div = (maximum + minimum).saturating_add(T::one()); // do not allow division by zero

    num / div
}

fn find_gradient<
    T: Ord + Default + Copy + NumOps<T> + Sub<Output = T> + Add<Output = T> + Div<Output = T>
>(
    data: &[T]
) -> T
{
    let mut minimum = T::max_val();
    let mut maximum = T::min_val();

    for datum in data
    {
        minimum = min(*datum, minimum);
        maximum = max(*datum, maximum);
    }
    maximum - minimum
}

#[inline(always)]
fn find_max<T: Ord + Copy + NumOps<T>>(data: &[T]) -> T
{
    let mut maximum = T::min_val();

    for datum in data
    {
        maximum = max(*datum, maximum);
    }
    maximum
}

fn find_mean<T>(data: &[T]) -> T
where
    T: Ord + Default + Copy + NumOps<T> + Add<Output = T> + Div<Output = T>,
    usize: std::convert::From<T>
{
    let mut maximum = usize::default();
    let len = data.len();

    for datum in data
    {
        maximum += usize::from(*datum);
    }
    T::from_usize(maximum / len)
}

pub fn spatial_ops<T>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize, height: usize,
    operations: StatisticOperations
) where
    T: Ord
        + Default
        + Copy
        + NumOps<T>
        + Sub<Output = T>
        + Add<Output = T>
        + Div<Output = T>
        + Debug,
    usize: std::convert::From<T>
{
    //pad here
    let padded_stuff = pad(
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
    let ptr = match operations
    {
        StatisticOperations::Contrast => find_contrast::<T>,
        StatisticOperations::Maximum => find_max::<T>,
        StatisticOperations::Gradient => find_gradient::<T>,
        StatisticOperations::Minimum => find_min::<T>,
        StatisticOperations::Mean => find_mean::<T>
    };

    spatial(&padded_stuff, out_channel, radius, width, height, ptr);
}
