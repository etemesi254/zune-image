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

/// Brighten struct
///
///
/// # Alpha channel
/// - Alpha  channel is ignored
///
/// # Example
/// ```
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::brighten::Brighten;
///  use zune_image::errors::ImageErrors;
/// // create gray image
/// let mut img = Image::fill::<u8>(128_u8,ColorSpace::RGB,100,100);
/// // make every pixel to be fully white
/// Brighten::new(1.0f32).execute(&mut img)?;
/// # Ok::<(),ImageErrors>(())
/// ```
#[derive(Default)]
pub struct Brighten {
    value: f32
}

impl Brighten {
    /// Create a new brightness filter
    ///
    /// # Arguments
    /// - value: Value to increase the channel values with, must be between -1 and 1, where 1 stands for maximum brightness
    ///  and -1 for darkness
    #[must_use]
    pub fn new(value: f32) -> Brighten {
        Brighten { value }
    }
}

impl OperationsTrait for Brighten {
    fn name(&self) -> &'static str {
        "Brighten"
    }

    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation
    )]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let max_val = image.depth().max_value();
        let depth = image.depth();

        for channel in image.channels_mut(true) {
            match depth.bit_type() {
                BitType::U8 => brighten(
                    channel.reinterpret_as_mut::<u8>()?,
                    self.value,
                    u8::try_from(max_val.clamp(0, 255)).unwrap()
                ),
                BitType::U16 => brighten(channel.reinterpret_as_mut::<u16>()?, self.value, max_val),
                BitType::F32 => brighten_f32(
                    channel.reinterpret_as_mut::<f32>()?,
                    self.value,
                    f32::from(max_val)
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
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
/// * `value`: Value to increase the channel values with, must be between -1 and 1, where 1 stands for maximum brightness
/// and -1 for darkness
/// * `_max_value`:  Currently unused
///
/// returns: ()
///
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn brighten<T: Copy + PartialOrd + NumOps<T> + Default>(
    channel: &mut [T], value: f32, _max_value: T
) {
    let t_min = T::min_val().to_f32();
    let t_max = T::max_val().to_f32();
    let scale_v = value.clamp(-1f32, 1f32) * (t_max - t_min);
    channel
        .iter_mut()
        .for_each(|x| *x = T::from_f32((x.to_f32() + scale_v).zclamp(t_min, t_max)));
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
