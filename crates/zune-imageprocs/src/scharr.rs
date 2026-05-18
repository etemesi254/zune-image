/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Scharr derivative filter
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::errors::ImageErrors::ImageOperationNotImplemented;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;
use crate::utils::{apply_gradient_3x3, execute_on};

/// Perform a scharr image derivative.
///
/// This operation calculates the gradient of the image,
/// which represents how quickly pixel values change from
/// one point to another in both the horizontal and vertical directions.
/// The magnitude and direction of the gradient can be used to detect edges in an image.
///
/// The matrix for scharr is
///
/// Gx matrix
/// ```text
///   -3, 0,  3,
///  -10, 0, 10,
///   -3, 0,  3
/// ```
/// Gy matrix
/// ```text
/// -3,-10,-3,
///  0,  0, 0,
///  3, 10, 3
/// ```
///
/// The window is a 3x3 window.
#[derive(Default, Copy, Clone)]
pub struct Scharr;

impl Scharr {
    /// Create a new scharr filter
    #[must_use]
    pub fn new() -> Scharr {
        Self
    }
}

impl OperationsTrait for Scharr {
    fn name(&self) -> &'static str {
        "Scharr"
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth().bit_type();
        let (width, height) = image.dimensions();

        let scharr_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let mut out_channel = Channel::new_with_bit_type(channel.len(), depth);
            match depth {
                BitType::U8 => scharr_int::<u8>(
                    channel.reinterpret_as()?,
                    out_channel.reinterpret_as_mut()?,
                    width,
                    height,
                ),

                BitType::U16 => scharr_int::<u16>(
                    channel.reinterpret_as()?,
                    out_channel.reinterpret_as_mut()?,
                    width,
                    height,
                ),

                BitType::F32 => scharr_float::<f32>(
                    channel.reinterpret_as()?,
                    out_channel.reinterpret_as_mut()?,
                    width,
                    height,
                ),
                _ => return Err(ImageOperationNotImplemented(self.name(), depth)),
            }
            *channel = out_channel;
            Ok(())
        };
        execute_on(scharr_fn, image, true)?;

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
#[rustfmt::skip]
const SCHARR_GX_I32: [i32; 9] = [
     -3, 0,  3,
    -10, 0, 10,
     -3, 0,  3,
];
#[rustfmt::skip]
const SCHARR_GY_I32: [i32; 9] = [
    -3, -10, -3,
     0,   0,  0,
     3,  10,  3,
];
#[rustfmt::skip]
const SCHARR_GX_F32: [f32; 9] = [
     -3.0, 0.0,  3.0,
    -10.0, 0.0, 10.0,
     -3.0, 0.0,  3.0,
];
#[rustfmt::skip]
const SCHARR_GY_F32: [f32; 9] = [
    -3.0, -10.0, -3.0,
     0.0,   0.0,  0.0,
     3.0,  10.0,  3.0,
];

pub fn scharr_int<T>(in_channel: &[T], out_channel: &mut [T], width: usize, height: usize)
where
    T: Default + NumOps<T> + Copy + Send + Sync,
    i32: From<T>,
{
    apply_gradient_3x3(
        in_channel,
        out_channel,
        width,
        height,
        &SCHARR_GX_I32,
        &SCHARR_GY_I32,
    );
}

pub fn scharr_float<T>(in_channel: &[T], out_channel: &mut [T], width: usize, height: usize)
where
    T: Default + NumOps<T> + Copy + Send + Sync,
    f32: From<T>,
{
    apply_gradient_3x3(
        in_channel,
        out_channel,
        width,
        height,
        &SCHARR_GX_F32,
        &SCHARR_GY_F32,
    );
}
