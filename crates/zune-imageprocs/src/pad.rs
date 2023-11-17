/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Methods used for duplicating pixels around the border
//!
//! This contains functions that make borders with specific types, such as constant values
//! or replicating values across the border
//!

/// Padding method to use
#[derive(Copy, Clone)]
pub enum PadMethod {
    /// Create a border with a constant value
    Constant,
    /// Create a border that duplicates the current pixel
    /// at the original image border to the new border
    ///
    /// ```text
    ///  a,b,c
    ///  d,e,f
    /// ```
    /// Becomes
    /// ```text
    ///   a,b,c
    /// a a,b,c c
    /// d d,e,f f
    ///   d,e,f
    /// ```
    Replicate
}

/// Pad pixels creating a buffer around actual pixels
///
/// This is mainly useful for convolutions and anything that
/// has conditions for edge pixels
///
///```text
///    padded width ──────────────────────────────►
/// │ ┌─────────────────────────────────────┐
/// │ │                                     │
/// │ │          PADDING                    │
/// │ │                                     │
/// │ │   (x,y)     out width               │
/// │ │     ┌────────────────────┐          │
/// │ │   o │                    │          │
/// │ │   u │                    │          │
/// │ │   t │                    │          │
/// │ │     │   IMAGE            │          │
/// │ │   h │                    │          │
/// │ │   e │                    │          │
/// │ │   i │                    │          │
/// │ │   g │                    │          │
/// │ │   h └────────────────────┘          │
/// │ │   t                                 │
/// │ │                                     │
/// ▼ │                                     │
///   └─────────────────────────────────────┘
/// ```
///
///
/// # Arguments
///  - pixels:Un-padded raw pixels
///  - width: Width of raw pixels
///  - height : Height of raw pixels
///  - pad_x: Number of columns to increase the width by.
///     The width is increased on both sides i.e left is padded by
///     pad_x and so is the right.
///  - pad_y: Number of rows to increase the height by
///  - method: Method to use for pad pixels.
///
/// # Returns:
///  - A vec containing padded pixels.
pub fn pad<T: Copy + Default>(
    pixels: &[T], width: usize, height: usize, pad_x: usize, pad_y: usize, method: PadMethod
) -> Vec<T> {
    match method {
        PadMethod::Constant => no_fill(pixels, width, height, pad_x, pad_y),
        PadMethod::Replicate => replicate(pixels, width, height, pad_x, pad_y)
    }
}

fn no_fill<T: Copy + Default>(
    pixels: &[T], width: usize, height: usize, pad_x: usize, pad_y: usize
) -> Vec<T> {
    let padded_w = width + pad_x * 2;
    let padded_h = height + pad_y * 2;

    let mut out_pixels = vec![T::default(); padded_h * padded_w];

    let start = pad_x;
    let end = padded_w - pad_x;
    // fill with black

    for (out, in_pix) in out_pixels
        .chunks_exact_mut(padded_w)
        .skip(pad_y)
        .take(height)
        .zip(pixels.chunks_exact(width))
    {
        out[start..end].copy_from_slice(in_pix);
    }
    out_pixels
}

fn replicate<T: Copy + Default>(
    pixels: &[T], width: usize, height: usize, pad_x: usize, pad_y: usize
) -> Vec<T> {
    let padded_w = width + pad_x * 2;
    let padded_h = height + pad_y * 2;

    let mut out_pixels = vec![T::default(); padded_h * padded_w];

    let start = pad_x;
    let end = padded_w - pad_x;
    // fill with black
    //
    //padded width ──────────────────────────────►
    // │ ┌─────────────────────────────────────┐
    // │ │                                     │
    // │ │          PADDING                    │
    // │ │                                     │
    // │ │   (x,y)     out width               │
    // │ │     ┌────────────────────┐          │
    // │ │   o │                    │          │
    // │ │   u │                    │          │
    // │ │   t │                    │          │
    // │ │     │   IMAGE            │          │
    // │ │   h │                    │          │
    // │ │   e │                    │          │
    // │ │   i │                    │          │
    // │ │   g │                    │          │
    // │ │   h └────────────────────┘          │
    // │ │   t                                 │
    // │ │                                     │
    // ▼ │                                     │
    //   └─────────────────────────────────────┘
    //

    // fill top row
    let first_row = &pixels[0..width];
    for out in out_pixels.chunks_exact_mut(padded_w).take(pad_y) {
        out[0..start].fill(first_row[0]);
        out[start..end].copy_from_slice(first_row);
        out[end..].fill(*first_row.last().unwrap_or(&T::default()));
    }
    // fill middle  rows
    for (out, in_pix) in out_pixels
        .chunks_exact_mut(padded_w)
        .skip(pad_y)
        .take(height)
        .zip(pixels.chunks_exact(width))
    {
        out[0..start].fill(in_pix[0]);
        out[start..end].copy_from_slice(in_pix);
        out[end..].fill(*in_pix.last().unwrap_or(&T::default()));
    }

    // fill bottom row
    let last_row = pixels.rchunks_exact(width).next().unwrap();
    for out in out_pixels.rchunks_exact_mut(padded_w).take(pad_y) {
        out[0..start].fill(last_row[0]);
        out[start..end].copy_from_slice(last_row);
        out[end..].fill(*last_row.last().unwrap_or(&T::default()));
    }
    out_pixels
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use crate::pad::{pad, PadMethod};

    #[bench]
    fn bench_pad_replicate(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;

        let pixels = vec![0_u8; width * height];
        let new_width = 960;
        let new_height = 1000;

        b.iter(|| {
            pad(
                &pixels,
                width,
                height,
                new_width,
                new_height,
                PadMethod::Replicate
            )
        });
    }

    #[bench]
    fn bench_pad_constant(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;

        let pixels = vec![0_u8; width * height];
        let new_width = 960;
        let new_height = 1000;
        b.iter(|| {
            pad(
                &pixels,
                width,
                height,
                new_width,
                new_height,
                PadMethod::Constant
            )
        });
    }
}
