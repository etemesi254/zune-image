/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;

/// Prefetch data at offset position
///
/// This uses prefetch intrinsics for a specific
/// platform to hint the CPU  that the data at that position
/// will be needed at a later time.
///
/// # Platform specific behaviour
/// - On x86, we use `_MM_HINT_T0` which prefetches to all levels of cache
/// hence it may cause cache pollution
///
/// # Arguments
///  - data: A long slice with some data not in the cache
///  - position: The position of data we expect to fetch that we think
/// is not in the cache.
#[inline(always)]
#[allow(dead_code, unused_variables)]
pub fn z_prefetch<T>(data: &[T], position: usize) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;
        unsafe {
            // we don't need to worry for this failing
            let ptr_position = data.as_ptr().add(position).cast::<i8>();

            _mm_prefetch::<_MM_HINT_T0>(ptr_position);
        }
    }
}

/// The position of the source image on the destination
#[derive(Copy, Clone, Debug)]
pub enum Gravity {
    /// Place the image so that it seems like it's from the
    /// center of the canvas
    Center,
    /// Place the image so that it is from the top end of the canvas
    TopLeft,
    /// Place the src image so that it appears from the right of the canvas
    TopRight,
    /// Place the image so that it appears from the bottom left of the canvas
    BottomLeft,
    /// Place the image so that it appears from the bottom right of the canvas
    BottomRight,
}

pub fn calculate_gravity(src_image: &Image, dst_image: &Image, gravity: Gravity) -> (usize, usize) {
    let (src_width, src_height) = src_image.dimensions();
    let (dst_width, dst_height) = dst_image.dimensions();

    return match gravity {
        Gravity::Center => {
            let dst_center_x = dst_width / 2;
            let dst_center_y = dst_height / 2;

            let src_center_x = src_width / 2;
            let src_center_y = src_height / 2;

            let orig_y = dst_center_y.saturating_sub(src_center_y);
            let orig_x = dst_center_x.saturating_sub(src_center_x);

            (orig_x, orig_y)
        }
        Gravity::TopLeft => (0, 0),
        Gravity::TopRight => (dst_width.saturating_sub(src_width), 0),
        Gravity::BottomLeft => (0, dst_height.saturating_sub(src_height)),
        Gravity::BottomRight => (
            dst_width.saturating_sub(src_width),
            dst_height.saturating_sub(src_height),
        ),
    };
}
/// A simple helper function to execute on threads
pub fn execute_on<T: Fn(&mut Channel) -> Result<(), ImageErrors> + Send + Sync>(
    function: T, image: &mut Image, ignore_alpha: bool,
) -> Result<(), ImageErrors> {
    #[cfg(feature = "threads")]
    {
        std::thread::scope(|s| {
            let mut t_results = vec![];
            for channel in image.channels_mut(ignore_alpha) {
                let result = s.spawn(|| function(channel));
                t_results.push(result);
            }

            t_results
                .into_iter()
                .map(|x| x.join().unwrap())
                .collect::<Result<Vec<()>, ImageErrors>>()
        })?;

        Ok(())
    }
    #[cfg(not(feature = "threads"))]
    {
        for channel in image.channels_mut(ignore_alpha) {
            function(channel)?;
        }
        Ok(())
    }
}

/// Apply a 3×3 gradient filter using two separable kernel passes (Gx, Gy).
///
/// Replicate-padding is handled by clamping neighbour coordinates, so no
/// intermediate allocation is needed.
///
/// The output pixel is `sqrt(Gx² + Gy²)`, clamped to `[T::min_val(), T::max_val()]`.
///
/// `Acc` is the accumulator type used during the dot-product (`i32` for integer
/// images, `f32` for float images). It must be `From<T>` and convertible back
/// via `T::from_f64`.
use crate::traits::NumOps;

/// Apply a 3×3 gradient filter (Gx, Gy → sqrt(Gx²+Gy²)).
///
/// Splits work into a clamped border ring and an unclamped interior.
/// The interior is parallelized when the `threads` feature is enabled.
pub fn apply_gradient_3x3<T, Acc>(
    src: &[T], dst: &mut [T], width: usize, height: usize, gx: &[Acc; 9], gy: &[Acc; 9],
) where
    T: NumOps<T> + Copy + Default + Send + Sync,
    Acc: Copy
        + Default
        + std::ops::Add<Output = Acc>
        + std::ops::Mul<Output = Acc>
        + Into<f64>
        + From<T>
        + Send
        + Sync,
{
    // Border pixels (top row, bottom row, left col, right col) — always clamped.
    // There are at most 2*(w+h) of them so the slow path barely shows up in profiles.
    let border_pixels = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| x == 0 || y == 0 || x == width - 1 || y == height - 1);

    for (x, y) in border_pixels {
        dst[y * width + x] = apply_at_clamped(src, width, height, x, y, gx, gy);
    }

    // Interior pixels never need clamping — all nine neighbours are valid.
    if width <= 2 || height <= 2 {
        return;
    }

    apply_interior(src, dst, width, height, gx, gy);
}

/// Compute one output pixel with full replicate-clamp boundary handling.
#[inline(always)]
fn apply_at_clamped<T, Acc>(
    src: &[T], width: usize, height: usize, cx: usize, cy: usize, gx: &[Acc; 9], gy: &[Acc; 9],
) -> T
where
    T: NumOps<T> + Copy + Default,
    Acc: Copy
        + Default
        + std::ops::Add<Output = Acc>
        + std::ops::Mul<Output = Acc>
        + Into<f64>
        + From<T>,
{
    let mut sum_x = Acc::default();
    let mut sum_y = Acc::default();
    for ky in 0..3usize {
        let sy = (cy + ky).saturating_sub(1).min(height - 1);
        for kx in 0..3usize {
            let sx = (cx + kx).saturating_sub(1).min(width - 1);
            let px = Acc::from(src[sy * width + sx]);
            let k = ky * 3 + kx;
            sum_x = sum_x + px * gx[k];
            sum_y = sum_y + px * gy[k];
        }
    }
    let gxf: f64 = sum_x.into();
    let gyf: f64 = sum_y.into();
    T::from_f64((gxf * gxf + gyf * gyf).sqrt()).zclamp(T::min_val(), T::max_val())
}

/// Process every interior pixel (y in 1..height-1, x in 1..width-1) with no clamping.
/// The two cfg blocks share identical logic; only the parallelism differs.
#[cfg(not(feature = "threads"))]
fn apply_interior<T, Acc>(
    src: &[T], dst: &mut [T], width: usize, height: usize, gx: &[Acc; 9], gy: &[Acc; 9],
) where
    T: NumOps<T> + Copy + Default + Send + Sync,
    Acc: Copy
        + Default
        + std::ops::Add<Output = Acc>
        + std::ops::Mul<Output = Acc>
        + Into<f64>
        + From<T>
        + Send
        + Sync,
{
    for y in 1..height - 1 {
        process_interior_row(src, &mut dst[y * width..(y + 1) * width], width, y, gx, gy);
    }
}

#[cfg(feature = "threads")]
fn apply_interior<T, Acc>(
    src: &[T], dst: &mut [T], width: usize, height: usize, gx: &[Acc; 9], gy: &[Acc; 9],
) where
    T: NumOps<T> + Copy + Default + Send + Sync,
    Acc: Copy
        + Default
        + std::ops::Add<Output = Acc>
        + std::ops::Mul<Output = Acc>
        + Into<f64>
        + From<T>
        + Send
        + Sync,
{
    use std::thread;

    // Skip the first and last output rows — already handled as border.
    let interior_dst = &mut dst[width..(height - 1) * width];

    thread::scope(|s| {
        let num_threads = thread::available_parallelism().map_or(4, std::num::NonZero::get);
        // Each chunk is a contiguous block of complete rows.
        let rows_per_chunk = ((height - 2).div_ceil(num_threads)).max(1);
        let chunk_size = rows_per_chunk * width;

        for (chunk_idx, out_chunk) in interior_dst.chunks_mut(chunk_size).enumerate() {
            let y_start = 1 + chunk_idx * rows_per_chunk; // absolute y of first row in chunk
            s.spawn(move || {
                for (local_row, out_row) in out_chunk.chunks_mut(width).enumerate() {
                    let y = y_start + local_row;
                    process_interior_row(src, out_row, width, y, gx, gy);
                }
            });
        }
    });
}

/// Compute one full interior row — no bounds checks needed.
#[inline(always)]
fn process_interior_row<T, Acc>(
    src: &[T], out_row: &mut [T], width: usize, y: usize, gx: &[Acc; 9], gy: &[Acc; 9],
) where
    T: NumOps<T> + Copy + Default,
    Acc: Copy
        + Default
        + std::ops::Add<Output = Acc>
        + std::ops::Mul<Output = Acc>
        + Into<f64>
        + From<T>,
{
    // Pre-load the three row pointers once per output row — the compiler can
    // keep them in registers and avoid re-computing `y * width` every pixel.
    let row0 = &src[(y - 1) * width..(y + 1) * width]; // rows y-1 and y share one slice
    let r0 = &src[(y - 1) * width..y * width];
    let r1 = &src[y * width..(y + 1) * width];
    let r2 = &src[(y + 1) * width..(y + 2) * width];

    for x in 1..width - 1 {
        // Explicitly unroll the 3×3 dot products.
        // Naming: p{row}{col} where row/col are offsets from centre.
        let p00 = Acc::from(r0[x - 1]);
        let p01 = Acc::from(r0[x]);
        let p02 = Acc::from(r0[x + 1]);

        let p10 = Acc::from(r1[x - 1]);
        let p11 = Acc::from(r1[x]);
        let p12 = Acc::from(r1[x + 1]);

        let p20 = Acc::from(r2[x - 1]);
        let p21 = Acc::from(r2[x]);
        let p22 = Acc::from(r2[x + 1]);

        let sum_x = p00 * gx[0]
            + p01 * gx[1]
            + p02 * gx[2]
            + p10 * gx[3]
            + p11 * gx[4]
            + p12 * gx[5]
            + p20 * gx[6]
            + p21 * gx[7]
            + p22 * gx[8];

        let sum_y = p00 * gy[0]
            + p01 * gy[1]
            + p02 * gy[2]
            + p10 * gy[3]
            + p11 * gy[4]
            + p12 * gy[5]
            + p20 * gy[6]
            + p21 * gy[7]
            + p22 * gy[8];

        let gxf: f64 = sum_x.into();
        let gyf: f64 = sum_y.into();
        out_row[x] = T::from_f64((gxf * gxf + gyf * gyf).sqrt()).zclamp(T::min_val(), T::max_val());
    }

    // Border pixels on this row were already written by apply_gradient_3x3.
    let _ = row0; // suppress unused warning from the pre-load experiment above
}
