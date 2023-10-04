/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::log::error;

use crate::pad::{pad, PadMethod};
use crate::traits::NumOps;
use crate::utils::z_prefetch;

fn convolve_3x3_inner<T>(in_array: &[T; 9], weights: &[f32; 9]) -> T
where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    let scale = 1.0; // 9.0;

    T::from_f32(
        in_array
            .iter()
            .zip(weights)
            .map(|(x, weight)| f32::from(*x) * weight)
            .sum::<f32>()
            * scale
    )
    .zclamp(T::min_val(), T::max_val())
}

fn convolve_5x5_inner<T>(in_array: &[T; 25], weights: &[f32; 25]) -> T
where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    let scale = 1.0; // / 25.0;
    T::from_f32(
        in_array
            .iter()
            .zip(weights)
            .map(|(x, weight)| f32::from(*x) * weight)
            .sum::<f32>()
            * scale
    )
    .zclamp(T::min_val(), T::max_val())
}

fn convolve_7x7_inner<T>(in_array: &[T; 49], weights: &[f32; 49]) -> T
where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    let scale = 1.0 / 49.0;
    T::from_f32(
        in_array
            .iter()
            .zip(weights)
            .map(|(x, weight)| f32::from(*x) * weight)
            .sum::<f32>()
            * scale
    )
    .zclamp(T::min_val(), T::max_val())
}

/// Convolve a matrix
pub fn convolve_3x3<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 9]
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    // pad input
    //pad here
    let padded_input = pad(in_channel, width, height, 1, 1, PadMethod::Replicate);

    spatial_NxN::<T, _, 1, 9>(
        &padded_input,
        out_channel,
        width,
        height,
        convolve_3x3_inner,
        weights
    );
}

pub fn convolve_5x5<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 25]
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    // pad input
    //pad here
    let padded_input = pad(in_channel, width, height, 2, 2, PadMethod::Replicate);

    spatial_NxN::<T, _, 2, 25>(
        &padded_input,
        out_channel,
        width,
        height,
        convolve_5x5_inner,
        weights
    );
}

pub fn convolve_7x7<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32; 49]
) where
    T: NumOps<T> + Copy + Default,
    f32: From<T>
{
    // pad input
    //pad here
    let padded_input = pad(in_channel, width, height, 3, 3, PadMethod::Replicate);

    spatial_NxN::<T, _, 3, 49>(
        &padded_input,
        out_channel,
        width,
        height,
        convolve_7x7_inner,
        weights
    );
}

/// Selects a convolve matrix
pub fn convolve<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, weights: &[f32]
) where
    T: NumOps<T> + Copy + Default,
    f32: std::convert::From<T>
{
    if weights.len() == 9 {
        convolve_3x3::<T>(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap()
        );
    } else if weights.len() == 25 {
        convolve_5x5::<T>(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap()
        );
    } else if weights.len() == 49 {
        convolve_7x7::<T>(
            in_channel,
            out_channel,
            width,
            height,
            weights.try_into().unwrap()
        );
    } else {
        debug_assert!(
            false,
            "Invalid array, expected 3x3,5x5 or 7x7 array for convolving"
        );
        error!("Not implemented, only works for 3x3, 5x5 and 7x7 arrays");
    }
}

/// A special spatial function that takes advantage of const generics to
/// speed up operations for convolve
#[allow(non_snake_case)]
fn spatial_NxN<T, F, const RADIUS: usize, const OUT_SIZE: usize>(
    in_channel: &[T], out_channel: &mut [T], width: usize, height: usize, function: F,
    values: &[f32; OUT_SIZE]
) where
    T: Default + Copy,
    F: Fn(&[T; OUT_SIZE], &[f32; OUT_SIZE]) -> T
{
    let old_width = width;
    let height = (RADIUS * 2) + height;
    let width = (RADIUS * 2) + width;

    assert_eq!(height * width, in_channel.len());

    let radius_size = (2 * RADIUS) + 1;

    let radius_loop = radius_size >> 1;

    let mut local_storage = [T::default(); OUT_SIZE];

    for y in radius_loop..height - radius_loop {
        for x in radius_loop..width - radius_loop {
            let iy = y - radius_loop;
            let ix = x - radius_loop;

            let mut i = 0;

            for ky in 0..radius_size {
                let iy_i = iy + ky;

                let in_slice = &in_channel[(iy_i * width) + ix..(iy_i * width) + ix + radius_size];
                z_prefetch(in_channel, (iy_i + 1) * width + ix);
                local_storage[i..i + radius_size].copy_from_slice(in_slice);
                z_prefetch(in_channel, (iy_i + 2) * width + ix);

                i += radius_size;
            }

            let result = function(&local_storage, values);

            out_channel[iy * old_width + ix] = result;
        }
    }
}

#[cfg(test)]
mod tests {
    use nanorand::Rng;

    use crate::convolve::{convolve_3x3, convolve_5x5, convolve_7x7};

    // test that 3x3 convolution works
    #[test]
    fn convolve_3x3_test() {
        let (width, height) = (100, 100);
        let mut data = vec![0u8; width * height];
        let mut out = vec![13; width * height];
        nanorand::WyRand::new().fill(&mut data);
        convolve_3x3(&data, &mut out, width, height, &[0.0; 9]);
        assert!(out.iter().all(|x| *x == 0));
    }

    #[test]
    fn convolve_5x5_test() {
        let (width, height) = (100, 100);
        let mut data = vec![0u8; width * height];
        let mut out = vec![13; width * height];
        nanorand::WyRand::new().fill(&mut data);
        convolve_5x5(&data, &mut out, width, height, &[0.0; 25]);
        assert!(out.iter().all(|x| *x == 0));
    }

    #[test]
    fn convolve_7x7_test() {
        let (width, height) = (100, 100);
        let mut data = vec![0u8; width * height];
        let mut out = vec![13; width * height];
        nanorand::WyRand::new().fill(&mut data);
        convolve_7x7(&data, &mut out, width, height, &[0.0; 49]);
        assert!(out.iter().all(|x| *x == 0));
    }
}
