/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Brighten operation
//!
//! # Algorithm
//!
//! The brightness algorithm is implemented as
//!
//! ```text
//! x = a + c
//! ```
//!
//! where
//! - `x`: New pixel
//! - `a`: Old pixel
//! - `c`: How bright to increase the pixel
//!
//! The `a+c` is saturating on the maximum value for the type
//!
use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;

#[derive(Default)]
pub struct Brighten {
    value: f32
}

impl Brighten {
    #[must_use]
    pub fn new(value: f32) -> Brighten {
        Brighten { value }
    }
}

impl OperationsTrait for Brighten {
    fn get_name(&self) -> &'static str {
        "Brighten"
    }

    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation
    )]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let max_val = image.get_depth().max_value();
        let depth = image.get_depth();

        for channel in image.get_channels_mut(true) {
            match depth.bit_type() {
                BitType::U8 => brighten(
                    channel.reinterpret_as_mut::<u8>()?,
                    self.value.clamp(0., 255.) as u8,
                    u8::try_from(max_val.clamp(0, 255)).unwrap()
                ),
                BitType::U16 => brighten(
                    channel.reinterpret_as_mut::<u16>()?,
                    self.value as u16,
                    max_val
                ),
                BitType::F32 => brighten_f32(
                    channel.reinterpret_as_mut::<f32>()?,
                    self.value,
                    f32::from(max_val)
                ),
                d => {
                    return Err(ImageErrors::ImageOperationNotImplemented(
                        self.get_name(),
                        d
                    ))
                }
            }
        }
        Ok(())
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::RGBA,
            ColorSpace::RGB,
            ColorSpace::LumaA,
            ColorSpace::Luma
        ]
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
/// Brighten operation
///
/// # Arguments
///
/// * `channel`: Input channel pixel, operates in place
/// * `value`: Value to increase the channel values with
/// * `_max_value`:  Currently unused
///
/// returns: ()
///
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn brighten<T: Copy + PartialOrd + NumOps<T> + Default>(
    channel: &mut [T], value: T, _max_value: T
) {
    channel
        .iter_mut()
        .for_each(|x| *x = (*x).saturating_add(value));
}

/// Brighten operation
///
/// # Arguments
///
/// * `channel`: Input channel pixel, operates in place
/// * `value`: Value to increase the channel values with
/// * `_max_value`:  Currently unused
///
/// returns: ()
///

pub fn brighten_f32(channel: &mut [f32], value: f32, max_value: f32) {
    channel
        .iter_mut()
        .for_each(|x| *x = (*x + value).clamp(0.0, max_value));
}
