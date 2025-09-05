/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Mathematical operations shared amongst functions

/// Implement fast integer division from
/// Daniel's Lemire fastmod.
///  
/// See [Faster Remainder by Direct Computation: Applications to Compilers and Software Libraries](https://arxiv.org/abs/1902.01961),
/// Software: Practice and Experience  49 (6), 2019.
#[inline(always)]
#[must_use]
pub fn compute_mod_u32(d: u64) -> u128 {
    // operator precedence will be the end of me,,
    return (u128::from(0xFFFF_FFFF_FFFF_FFFF_u64) / u128::from(d)) + 1;
}

// /// Implement fast integer division from
// /// Daniel's Lemire fastmod.
// ///
// /// See [Faster Remainder by Direct Computation: Applications to Compilers and Software Libraries](https://arxiv.org/abs/1902.01961),
// /// Software: Practice and Experience  49 (6), 2019.
// #[inline(always)]
// #[must_use]
// fn mul128_u32(low_bits: u64, d: u32) -> u64 {
//     return ((u128::from(low_bits) * u128::from(d)) >> 64) as u64;
// }

/// Fast division of u32 numbers
///  
/// Implement fast integer division from
/// See [Faster Remainder by Direct Computation: Applications to Compilers and Software Libraries](https://arxiv.org/abs/1902.01961),
/// Software: Practice and Experience  49 (6), 2019.
#[inline(always)]
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn fastdiv_u32(a: u32, m: u128) -> u32 {
    return ((m * u128::from(a)) >> 64) as u32;
}

/// Test fast_div works
#[test]
fn test_u8_div() {
    for i in 1..1000 {
        let num = compute_mod_u32(i);
        let divisor = fastdiv_u32(13459, num);

        assert_eq!(13459 / i, u64::from(divisor));
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn round_f32(x: f32) -> f32 {
    #[cfg(feature = "libm")]
    {
        libm::roundf(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f32::round(x)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn round_f64(x: f64) -> f64 {
    #[cfg(feature = "libm")]
    {
        libm::round(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f64::round(x)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn exp_f64(x: f64) -> f64 {
    #[cfg(feature = "libm")]
    {
        libm::exp(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f64::exp(x)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn sqrt_f32(x: f32) -> f32 {
    #[cfg(feature = "libm")]
    {
        libm::sqrtf(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f32::sqrt(x)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn sqrt_f64(x: f64) -> f64 {
    #[cfg(feature = "libm")]
    {
        libm::sqrt(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f64::sqrt(x)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn powf_f32(base: f32, exp: f32) -> f32 {
    #[cfg(feature = "libm")]
    {
        libm::powf(base, exp)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f32::powf(base, exp)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn sin_f32(x: f32) -> f32 {
    #[cfg(feature = "libm")]
    {
        libm::sinf(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f32::sin(x)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn cos_f32(x: f32) -> f32 {
    #[cfg(feature = "libm")]
    {
        libm::cosf(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f32::cos(x)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn powi_f32(base: f32, exp: i32) -> f32 {
    #[cfg(feature = "libm")]
    {
        powf_f32(base, exp as f32)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f32::powi(base, exp)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn trunc_f32(x: f32) -> f32 {
    #[cfg(feature = "libm")]
    {
        libm::truncf(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f32::trunc(x)
    }
}

#[inline(always)]
#[must_use]
pub(crate) fn floor_f32(x: f32) -> f32 {
    #[cfg(feature = "libm")]
    {
        libm::floorf(x)
    }

    #[cfg(all(not(feature = "libm"), feature = "std"))]
    {
        f32::floor(x)
    }
}
