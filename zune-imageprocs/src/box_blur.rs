/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::f32;

use zune_core::bit_depth::BitType;
use zune_core::log::{trace, warn};
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::mathops::{compute_mod_u32, fastdiv_u32};
use crate::traits::NumOps;
use crate::transpose;

/// Perform a box blur
///
/// Radius is a measure of how many
/// pixels to include in the box blur.
///
/// The greater the radius, the more pronounced the box blur
///
/// This operation is multithreaded capable
#[derive(Default)]
pub struct BoxBlur {
    radius: usize
}

impl BoxBlur {
    /// Create a new blur operation.
    ///
    /// # Arguments
    /// - radius: The radius of the blur, larger the value the more pronounced the blur
    #[must_use]
    pub fn new(radius: usize) -> BoxBlur {
        BoxBlur { radius }
    }
}

impl OperationsTrait for BoxBlur {
    fn get_name(&self) -> &'static str {
        "Box blur"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.get_dimensions();

        let depth = image.get_depth();

        #[cfg(feature = "threads")]
        {
            trace!("Running box blur in multithreaded mode");
            std::thread::scope(|s| {
                let mut errors = vec![];
                // blur each channel on a separate thread
                for channel in image.get_channels_mut(false) {
                    let result = s.spawn(|| match depth.bit_type() {
                        BitType::U16 => {
                            let mut scratch_space = vec![0; width * height];
                            let data = channel.reinterpret_as_mut::<u16>()?;
                            box_blur_u16(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }
                        BitType::U8 => {
                            let mut scratch_space = vec![0; width * height];
                            let data = channel.reinterpret_as_mut::<u8>()?;
                            box_blur_u8(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }

                        BitType::F32 => {
                            let mut scratch_space = vec![0.0; width * height];
                            let data = channel.reinterpret_as_mut::<f32>()?;
                            box_blur_f32(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }
                        d => return Err(ImageErrors::ImageOperationNotImplemented("box_blur", d))
                    });
                    errors.push(result);
                }
                errors
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running box blur in single threaded mode");

            match depth.bit_type() {
                BitType::U16 => {
                    let mut scratch_space = vec![0; width * height];

                    for channel in image.get_channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<u16>()?;
                        box_blur_u16(data, &mut scratch_space, width, height, self.radius);
                    }
                }
                BitType::U8 => {
                    let mut scratch_space = vec![0; width * height];

                    for channel in image.get_channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<u8>()?;
                        box_blur_u8(data, &mut scratch_space, width, height, self.radius);
                    }
                }

                BitType::F32 => {
                    let mut scratch_space = vec![0.0; width * height];

                    for channel in image.get_channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<f32>()?;
                        box_blur_f32(data, &mut scratch_space, width, height, self.radius);
                    }
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented("box_blur", d))
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

pub fn box_blur_u16(
    in_out_image: &mut [u16], scratch_space: &mut [u16], width: usize, height: usize, radius: usize
) {
    if width == 0 || radius <= 1 {
        warn!("Box blur with radius less than or equal to 1 does nothing");
        return;
    }
    box_blur_inner(in_out_image, scratch_space, width, radius);
    transpose::transpose_u16(scratch_space, in_out_image, width, height);
    box_blur_inner(in_out_image, scratch_space, height, radius);
    transpose::transpose_u16(scratch_space, in_out_image, height, width);
}

pub fn box_blur_u8(
    in_out_image: &mut [u8], scratch_space: &mut [u8], width: usize, height: usize, radius: usize
) {
    if width == 0 || radius <= 1 {
        warn!("Box blur with radius less than or equal to 1 does nothing");
        return;
    }
    box_blur_inner(in_out_image, scratch_space, width, radius);
    transpose::transpose_u8(scratch_space, in_out_image, width, height);
    box_blur_inner(in_out_image, scratch_space, height, radius);
    transpose::transpose_u8(scratch_space, in_out_image, height, width);
}

pub fn box_blur_f32(
    in_out_image: &mut [f32], scratch_space: &mut [f32], width: usize, height: usize, radius: usize
) {
    if width == 0 || radius <= 1 {
        warn!("Box blur with radius less than or equal to 1 does nothing");
        return;
    }
    box_blur_f32_inner(in_out_image, scratch_space, width, radius);
    transpose::transpose_generic(scratch_space, in_out_image, width, height);
    box_blur_f32_inner(in_out_image, scratch_space, height, radius);
    transpose::transpose_generic(scratch_space, in_out_image, height, width);
}

#[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
pub(crate) fn box_blur_inner<T>(in_image: &[T], out_image: &mut [T], width: usize, radius: usize)
where
    T: Copy + NumOps<T>,
    u32: std::convert::From<T>
{
    let radius = (radius * 2) + 1;
    // 1D-Box blurs can be seen as the average of radius pixels iterating
    // through a window
    // A box blur therefore is
    //
    // pix[x,y]= (pix[x-r/2,y]...+pix[x,y]+...pix[x+r/2,y])/r
    //
    // The naive operation is slow, due to a lot of reasons, so here we use a slightly more
    // optimized version
    //
    // One thing to see is that 1D box blurs are independent per width stride
    // ie. calculating row y is independent of calculating row y+1, with this info
    // we can do a bit of loop unrolling to better utilize ILP.
    //
    // Furthermore, notice we are dividing by r, which is a constant across all rows,
    // But division is a slow instruction, hence we can replace it with multiplication by some
    // weird constant, that eliminates that in the inner loop, credits to Daniel Lemire's fastmod for that
    //
    // Further more there is no need to sum up a window per iteration, we can simply implement it by looking at what is changing
    // For any iteration the sum is window[n], and for n+1 sum is window[n+1], but what changed was nothing
    // i.e what changed was windows[n-r/2] was dropped and windows [n+r/2] was added.
    // So if we keep the terms window[n-r/2] and windows [n+r/2] the summing becomes
    // sum(windows[n]) = a - windows[r]-windows[0]
    // where a is sum of chunk[0..r], (first of the array), we can keep updating a during the loop
    // and we have a window sum!

    if width <= 1 || radius <= 1 {
        // repeated here for the optimizer
        return;
    }
    let radius = radius.min(width);
    let m_radius = compute_mod_u32((radius + 1) as u64);

    for (stride_in, stride_out) in in_image
        .chunks_exact(width)
        .zip(out_image.chunks_exact_mut(width))
    {
        let half_radius = (radius + 1) / 2;

        let mut accumulator: u32 = stride_in[..half_radius].iter().map(|x| u32::from(*x)).sum();

        accumulator += (half_radius as u32) * u32::from(stride_in[0]);

        for (data_in, data_out) in stride_in[half_radius..]
            .iter()
            .zip(stride_out.iter_mut())
            .take(half_radius)
        {
            accumulator += u32::from(*data_in);
            accumulator -= u32::from(stride_in[0]);

            *data_out = T::from_u32(fastdiv_u32(accumulator, m_radius));
        }

        let mut window_slide = 0;
        let mut mask = 0;

        for (window_in, data_out) in stride_in
            .windows(radius)
            .zip(stride_out[half_radius..].iter_mut())
        {
            accumulator = accumulator.wrapping_sub(window_slide);
            accumulator = accumulator.wrapping_add(u32::from(*window_in.last().unwrap()) & mask);

            mask = u32::MAX;
            window_slide = u32::from(window_in[0]);

            *data_out = T::from_u32(fastdiv_u32(accumulator, m_radius));
        }

        let edge_len = stride_out.len() - half_radius;

        let end_stride = &mut stride_out[edge_len..];
        let last_item = u32::from(*stride_in.last().unwrap());

        for (data_in, data_out) in stride_in[edge_len..]
            .iter()
            .zip(end_stride)
            .take(half_radius)
        {
            accumulator = accumulator.wrapping_sub(u32::from(*data_in));
            accumulator = accumulator.wrapping_add(last_item);

            *data_out = T::from_u32(fastdiv_u32(accumulator, m_radius));
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::cast_precision_loss
)]
pub(crate) fn box_blur_f32_inner(
    in_image: &[f32], out_image: &mut [f32], width: usize, radius: usize
) {
    if width <= 1 || radius <= 1 {
        // repeated here for the optimizer
        return;
    }
    let radius = radius.min(width);
    let m_radius = 1.0 / ((radius + 1) as f32);

    for (stride_in, stride_out) in in_image
        .chunks_exact(width)
        .zip(out_image.chunks_exact_mut(width))
    {
        let half_radius = (radius + 1) / 2;

        let mut accumulator: f32 = stride_in[..half_radius].iter().copied().sum();

        accumulator += (half_radius as f32) * stride_in[0];

        for (data_in, data_out) in stride_in[half_radius..]
            .iter()
            .zip(stride_out.iter_mut())
            .take(half_radius)
        {
            accumulator += *data_in;
            accumulator -= stride_in[0];

            *data_out = accumulator * m_radius;
        }

        let mut window_slide = 0.0;
        let mut mask = 0.0;

        for (window_in, data_out) in stride_in
            .windows(radius)
            .zip(stride_out[half_radius..].iter_mut())
        {
            accumulator -= window_slide;
            accumulator += (*window_in.last().unwrap()) * mask;

            mask = 1.0;
            window_slide = window_in[0];

            *data_out = accumulator * m_radius;
        }

        let edge_len = stride_out.len() - half_radius;

        let end_stride = &mut stride_out[edge_len..];
        let last_item = *stride_in.last().unwrap();

        for (data_in, data_out) in stride_in[edge_len..]
            .iter()
            .zip(end_stride)
            .take(half_radius)
        {
            accumulator -= *data_in;
            accumulator += last_item;

            *data_out = accumulator * m_radius;
        }
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use crate::box_blur::{box_blur_u16, box_blur_u8};

    #[bench]
    fn bench_box_blur_u16(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let radius = 10;
        let dimensions = width * height;
        let mut in_vec = vec![255; dimensions];
        let mut scratch_space = vec![0; dimensions];

        b.iter(|| {
            box_blur_u16(&mut in_vec, &mut scratch_space, width, height, radius);
        });
    }

    #[bench]
    fn bench_box_blur_u8(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let radius = 10;
        let dimensions = width * height;
        let mut in_vec = vec![255; dimensions];
        let mut scratch_space = vec![0; dimensions];

        b.iter(|| {
            box_blur_u8(&mut in_vec, &mut scratch_space, width, height, radius);
        });
    }
}

#[test]
fn test_blur() {
    let width = 800;
    let height = 800;
    let radius = 10;
    let dimensions = width * height;
    let mut in_vec = vec![255; dimensions];
    let mut scratch_space = vec![0; dimensions];

    box_blur_u16(&mut in_vec, &mut scratch_space, width, height, radius);
}
