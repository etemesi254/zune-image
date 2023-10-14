/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use crate::traits::NumOps;

#[derive(Copy, Clone, Debug)]
pub enum ThresholdMethod {
    Binary,
    BinaryInv,
    ThreshTrunc,
    ThreshToZero
}

impl ThresholdMethod {
    pub fn from_string_result(input: &str) -> Result<Self, String> {
        match input
        {
            "binary" => Ok(Self::Binary),
            "binary_inv" => Ok(Self::BinaryInv),
            "thresh_trunc" => Ok(Self::ThreshTrunc),
            "thresh_to_zero" => Ok(Self::ThreshToZero),
            _ => Err("Unknown threshold type,accepted values are binary,binary_inv,thresh_trunc,thresh_to_zero".to_string()),
        }
    }
}

#[rustfmt::skip]
pub fn threshold<T>(in_channel: &mut [T], threshold: T, method: ThresholdMethod)
    where
        T: NumOps<T> + Copy + PartialOrd
{
    let max = T::max_val();
    let min = T::min_val();
    match method
    {
        ThresholdMethod::Binary =>
            {
                for x in in_channel.iter_mut()
                {
                    *x = if *x > threshold { max } else { min };
                }
            }
        ThresholdMethod::BinaryInv =>
            {
                for x in in_channel.iter_mut()
                {
                    *x = if *x > threshold { min } else { max };
                }
            }
        ThresholdMethod::ThreshTrunc =>
            {
                for x in in_channel.iter_mut()
                {
                    *x = if *x > threshold { threshold } else { *x };
                }
            }
        ThresholdMethod::ThreshToZero =>
            {
                for x in in_channel.iter_mut()
                {
                    *x = if *x > threshold { threshold } else { T::min_val() }
                }
            }
    }
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    #[bench]
    fn threshold_scalar_u8(b: &mut test::Bencher) {
        use crate::threshold::threshold;

        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0_u8; dimensions];

        b.iter(|| {
            threshold(&mut c1, 10, crate::threshold::ThresholdMethod::BinaryInv);
        });
    }

    #[bench]
    fn threshold_scalar_u16(b: &mut test::Bencher) {
        use crate::threshold::threshold;

        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0_u16; dimensions];

        b.iter(|| {
            threshold(&mut c1, 10, crate::threshold::ThresholdMethod::BinaryInv);
        });
    }
}
