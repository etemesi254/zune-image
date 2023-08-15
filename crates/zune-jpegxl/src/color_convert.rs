/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Color conversion routines
//!
//! This module provides two functions
//!
//! - `RGB[a]` -> `YCoCg[a]`
//!
//! - `Luma[A]` to  `Luma[A]`
//!
//! For both 8 bit and 16 bit images
//!
//! 16 bit images are treated as `&[u8]` bytes in **native endian**
use core::ops::{Add, Shr, Sub};

fn convert_rgb_to_ycocg<T>(r: T, g: T, b: T, y: &mut T, co: &mut T, cg: &mut T)
where
    T: Add<Output = T> + Sub<Output = T> + Shr<u8, Output = T> + Copy
{
    *co = r - b;
    let tmp = b + (*co >> 1);
    *cg = g - tmp;
    *y = tmp + (*cg >> 1);
}

pub fn fill_row_g8<T>(pixels: &[u8], oxs: usize, luma: &mut [T])
where
    T: From<u8>
{
    for (rg, lm) in pixels.iter().zip(luma).take(oxs) {
        *lm = T::from(*rg);
    }
}

pub fn fill_row_g16<T>(pixels: &[u8], oxs: usize, luma: &mut [T])
where
    T: From<i16>
{
    for (rg, lm) in pixels.chunks_exact(2).zip(luma).take(oxs) {
        let value = i16::from_ne_bytes([rg[0], rg[1]]);
        *lm = T::from(value);
    }
}

pub fn fill_row_ga8<T>(pixels: &[u8], oxs: usize, luma: &mut [T], alpha: &mut [T])
where
    T: From<u8>
{
    for ((rg, lm), am) in pixels.chunks_exact(2).zip(luma).zip(alpha).take(oxs) {
        *lm = T::from(rg[0]);
        *am = T::from(rg[1]);
    }
}

pub fn fill_row_ga16<T>(pixels: &[u8], oxs: usize, luma: &mut [T], alpha: &mut [T])
where
    T: From<i16>
{
    for ((rg, lm), am) in pixels.chunks_exact(4).zip(luma).zip(alpha).take(oxs) {
        let luma_bits = i16::from_ne_bytes([rg[0], rg[1]]);
        let alpha_bits = i16::from_ne_bytes([rg[2], rg[3]]);

        *lm = T::from(luma_bits);
        *am = T::from(alpha_bits);
    }
}

pub fn fill_row_rgb8<T>(pixels: &[u8], oxs: usize, y: &mut [T], co: &mut [T], cg: &mut [T])
where
    T: Add<Output = T> + Sub<Output = T> + Shr<u8, Output = T> + Copy + From<u8>
{
    for (((rgb, y), co), cg) in pixels.chunks_exact(3).take(oxs).zip(y).zip(co).zip(cg) {
        let r = rgb[0].into();
        let g = rgb[1].into();
        let b = rgb[2].into();

        convert_rgb_to_ycocg(r, g, b, y, co, cg);
    }
}

pub fn fill_row_rgb16<T>(pixels: &[u8], oxs: usize, y: &mut [T], co: &mut [T], cg: &mut [T])
where
    T: Add<Output = T>
        + Sub<Output = T>
        + Shr<u8, Output = T>
        + Copy
        + Into<i32>
        + TryFrom<i32>
        + Default
{
    for (((rgb, y), co), cg) in pixels.chunks_exact(6).take(oxs).zip(y).zip(co).zip(cg) {
        let r = u16::from_ne_bytes([rgb[0], rgb[1]]).into();
        let g = u16::from_ne_bytes([rgb[2], rgb[3]]).into();
        let b = u16::from_ne_bytes([rgb[4], rgb[5]]).into();

        let mut y_x = (*y).into();
        let mut co_x = (*co).into();
        let mut cg_x = (*cg).into();

        convert_rgb_to_ycocg::<i32>(r, g, b, &mut y_x, &mut co_x, &mut cg_x);

        *y = T::try_from(y_x).unwrap_or_default();
        *co = T::try_from(co_x).unwrap_or_default();
        *cg = T::try_from(cg_x).unwrap_or_default();
    }
}

pub fn fill_row_rgba8<T>(
    pixels: &[u8], oxs: usize, y: &mut [T], co: &mut [T], cg: &mut [T], alpha: &mut [T]
) where
    T: Add<Output = T> + Sub<Output = T> + Shr<u8, Output = T> + Copy + From<u8>
{
    for ((((rgb, y), co), cg), a) in pixels
        .chunks_exact(4)
        .take(oxs)
        .zip(y)
        .zip(co)
        .zip(cg)
        .zip(alpha)
    {
        let r = rgb[0].into();
        let g = rgb[1].into();
        let b = rgb[2].into();

        convert_rgb_to_ycocg(r, g, b, y, co, cg);
        *a = rgb[3].into();
    }
}

pub fn fill_row_rgba16<T>(
    pixels: &[u8], oxs: usize, y: &mut [T], co: &mut [T], cg: &mut [T], alpha: &mut [T]
) where
    T: Add<Output = T>
        + Sub<Output = T>
        + Shr<u8, Output = T>
        + Copy
        + Into<i32>
        + TryFrom<i32>
        + Default
{
    for ((((rgb, y), co), cg), ca) in pixels
        .chunks_exact(8)
        .take(oxs)
        .zip(y)
        .zip(co)
        .zip(cg)
        .zip(alpha)
    {
        let r = u16::from_ne_bytes([rgb[0], rgb[1]]).into();
        let g = u16::from_ne_bytes([rgb[2], rgb[3]]).into();
        let b = u16::from_ne_bytes([rgb[4], rgb[5]]).into();
        let a = u16::from_ne_bytes([rgb[6], rgb[7]]).into();

        let mut y_x = (*y).into();
        let mut co_x = (*co).into();
        let mut cg_x = (*cg).into();

        convert_rgb_to_ycocg::<i32>(r, g, b, &mut y_x, &mut co_x, &mut cg_x);

        *y = T::try_from(y_x).unwrap_or_default();
        *co = T::try_from(co_x).unwrap_or_default();
        *cg = T::try_from(cg_x).unwrap_or_default();
        *ca = T::try_from(a).unwrap_or_default();
    }
}
