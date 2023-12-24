/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Alpha pre-multiplication routines
//!
//! This module contains routines to convert to and from
//! premultiplied alpha
//!
//!
//!  # Algorithm
//! - To compute premultiplied alpha, we simply multiply alpha by color channel
//!  so assuming float to convert from straight alpha to premultiplied alpha we can use
//!  `RGB*a`, here `a` is between 0.0-1.0 (RGB are three color components, a multiplied
//!  them separately)
//!  We can handle this in integers, e.g for u8, the mapping `0.0-`1.0` can be scaled by `255` to
//!  get alpha value in u8, and then doing alpha-premultiplication can be presented by the formula
//!
//! - `(RGB*255)/a`, we first scale the value by 255 in order for division to work since now the mapping
//! is (255..65535), if your `R` value is 1 and alpha is 255, you get `(1*255)/255`, and similarly, if your value is `255`
//! and alpha is 128 you get `(255*255)/128`. This allows us to actually carry out alpha pre-multiplication in integers
//!
//! - But division is a slow instruction and Rust tends to add checks for zero (and appropriate panics)
//! hence it's either badly vectorized or not vectorized, furthermore it's hard to parallelize it
//! in the instruction cache pipeline, which means the simple operation reduces speed.
//!
//! #### A solution
//!
//!  - The solution here is that for integers, we are entirely in a fixed bounds, e.g for u8, we are
//! bounded by `0..255`, always, and u16, we are bound to `0..65535`, (PS this doesn't apply for floats)
//! and we can apply another optimization, namely Daniel's Lemire [fastmod](https://github.com/lemire/fastmod)
//! so to compute special constants during runtime that can be used to divide via reciprocal multiplication
//! so now to create premultiplied alpha, we simply the code becomes
//!
//! - Compute constants
//! - Iterate over source channel and alpha,
//! - Lookup special constant (`c` ) for the alpha value
//! - Multiply that constant `c` with channel value and take top bits
//! [`fastdiv_u32`]
//! -

use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::colorspace::ColorSpace;
use zune_core::log::warn;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::OperationsTrait;

use crate::mathops::{compute_mod_u32, fastdiv_u32};

mod std_simd;

/// Carry out alpha pre-multiply and un-premultiply
///
/// The type of transform is specified.
///
/// Note that some operations are lossy,
/// due to the nature of the operation multiplying and dividing values.
/// Where alpha is to big to fit into target integer, or zero, there will
/// be loss of image quality.
#[derive(Copy, Clone)]
pub struct PremultiplyAlpha {
    to: AlphaState
}

impl PremultiplyAlpha {
    /// Create a new alpha pre-multiplication operation.
    ///
    /// It can be used to convert from pre-multiplied alpha to
    /// normal alpha or vice-versa
    #[must_use]
    pub fn new(to: AlphaState) -> PremultiplyAlpha {
        PremultiplyAlpha { to }
    }
}

impl OperationsTrait for PremultiplyAlpha {
    fn name(&self) -> &'static str {
        "pre-multiply alpha"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        if !image.colorspace().has_alpha() {
            warn!("Image colorspace indicates no alpha channel, this operation is a no-op");
            return Ok(());
        }

        let colorspaces = image.colorspace();
        let alpha_state = image.metadata().alpha();

        if alpha_state == self.to {
            warn!("Alpha is already in required mode, exiting");
            return Ok(());
        }

        let bit_type = image.depth();

        for image_frame in image.frames_mut() {
            // read colorspace
            // split between alpha and color channels
            let (color_channels, alpha) = {
                if colorspaces == ColorSpace::ARGB {
                    // special for our guy :)
                    let im = image_frame.channels_vec();
                    // a is first channel, colors come later, so split at that
                    let (alpha, channels) = im.split_at_mut(1);

                    (channels, alpha)
                } else {
                    image_frame
                        .channels_mut(colorspaces, false)
                        .split_at_mut(colorspaces.num_components() - 1)
                }
            };

            assert_eq!(alpha.len(), 1);

            // create static tables
            let u8_table = create_unpremul_table_u8();
            let mut u16_table = vec![];

            if bit_type == BitDepth::Sixteen {
                u16_table = create_unpremul_table_u16();
            }
            for channel in color_channels {
                // from alpha channel, read
                match (alpha_state, self.to) {
                    (AlphaState::NonPreMultiplied, AlphaState::PreMultiplied) => match bit_type {
                        BitDepth::Eight => {
                            premultiply_u8(
                                channel.reinterpret_as_mut()?,
                                alpha[0].reinterpret_as()?
                            );
                        }
                        BitDepth::Sixteen => {
                            premultiply_u16(
                                channel.reinterpret_as_mut()?,
                                alpha[0].reinterpret_as()?
                            );
                        }

                        BitDepth::Float32 => premultiply_f32(
                            channel.reinterpret_as_mut()?,
                            alpha[0].reinterpret_as()?
                        ),
                        d => {
                            return Err(ImageErrors::ImageOperationNotImplemented(
                                self.name(),
                                d.bit_type()
                            ))
                        }
                    },
                    (AlphaState::PreMultiplied, AlphaState::NonPreMultiplied) => match bit_type {
                        BitDepth::Eight => {
                            unpremultiply_u8(
                                channel.reinterpret_as_mut()?,
                                alpha[0].reinterpret_as()?,
                                &u8_table
                            );
                        }
                        BitDepth::Sixteen => {
                            unpremultiply_u16(
                                channel.reinterpret_as_mut()?,
                                alpha[0].reinterpret_as()?,
                                &u16_table
                            );
                        }

                        BitDepth::Float32 => unpremultiply_f32(
                            channel.reinterpret_as_mut()?,
                            alpha[0].reinterpret_as()?
                        ),
                        d => {
                            return Err(ImageErrors::ImageOperationNotImplemented(
                                self.name(),
                                d.bit_type()
                            ))
                        }
                    },
                    (_, _) => return Err(ImageErrors::GenericStr("Could not pre-multiply alpha"))
                }
            }
        }

        // update metadata
        image.metadata_mut().set_alpha(self.to);

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::F32, BitType::U16, BitType::U8]
    }
}

/// Create the fastdiv table for u8 division
///
/// Useful for speeding up un-pre-multiplying alpha
#[allow(clippy::needless_range_loop)]
#[must_use]
pub fn create_unpremul_table_u8() -> [u128; 256] {
    let mut array = [0; 256];

    for i in 1..256 {
        array[i] = compute_mod_u32(i as u64);
    }

    array
}

/// Create a fastdiv table for u16 division
///
/// Useful for speeding up un-pre-multiplying alpha
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn create_unpremul_table_u16() -> Vec<u128> {
    let mut array = vec![0; 65536];

    for i in 1..65536 {
        array[i] = compute_mod_u32(i as u64);
    }

    array
}

///  Simple pre-multiply alpha implementation
///
/// # Arguments
///
/// * `input`:  Input array which contains non-premultiplied alpha
/// * `alpha`:  Alpha values corresponding to pre-multiplied alpha
///
/// Items in input are modified in place.
#[allow(clippy::cast_possible_truncation)]
pub fn premultiply_u8(input: &mut [u8], alpha: &[u8]) {
    const MAX_VALUE: u16 = 255;

    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        let temp = (u16::from(*al) * u16::from(*color)) + 0x80;

        *color = ((temp + (temp >> 8)) / MAX_VALUE) as u8;
    });
}

/// Specialized pre_multiply for u16
///
/// # Arguments
///
/// * `input`: Input array, values modified in place
/// * `alpha`: Alpha for associated values
///
/// returns: Array modified in place
#[allow(clippy::cast_possible_truncation)]
pub fn premultiply_u16(input: &mut [u16], alpha: &[u16]) {
    const MAX_VALUE: u32 = 65535;

    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        let temp = (u32::from(*al) * u32::from(*color)) + ((MAX_VALUE + 1) / 2);
        *color = ((temp + (temp >> 16)) / MAX_VALUE) as u16;
    });
}

/// Remove effects of pre-multiplied alpha
///
/// # Arguments
///
/// * `input` : Input u8 values which are pre-multiplied with alpha
/// * `alpha` : The alpha value pre-multiplied
pub fn unpremultiply_u8(input: &mut [u8], alpha: &[u8], premul_table: &[u128; 256]) {
    // we did         pa = (color * alpha)/255,
    // to undo we do  pb = (color * 255  )/alpha

    const MAX_VALUE: u32 = 255;

    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        let associated_alpha = premul_table[usize::from(*al)];
        *color = u8::try_from(fastdiv_u32(
            u32::from(*color) * MAX_VALUE + (u32::from(*al) / 2),
            associated_alpha
        ))
        .unwrap_or(u8::MAX);
    });
}

/// Undo effect of alpha multiplication
///
/// # Arguments
///
/// * `input`: Array with pre-multiplied coefficients
/// * `alpha`:  The alpha value which was pre-multiplied
/// * `premul_table`:  A table of coefficients which contains special
///   numbers that do reciprocal multiplication, generated by
///   [create_unpremul_table_u16]
///
/// Array is modified in place
pub fn unpremultiply_u16(input: &mut [u16], alpha: &[u16], premul_table: &[u128]) {
    // we did         pa = (color * alpha)/65535,
    // to undo we do  pb = (color * 65535)/alpha

    const MAX_VALUE: u32 = 65535;

    debug_assert!(premul_table.len() > 65535);
    if premul_table.len() < 65536 {
        // this invariant ensures that we remove bounds check from below loop
        // u16 range from 0..65535, hence premul_table[usize::from(u16)] will
        // always be in bounds if this is true
        return;
    }

    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        let associated_alpha = premul_table[usize::from(*al)];

        *color = u16::try_from(fastdiv_u32(
            u32::from(*color) * MAX_VALUE + (u32::from(*al) / 2),
            associated_alpha
        ))
        .unwrap_or(u16::MAX);
    });
}

/// Premultiply a f32 array with the associated alpha
///
/// # Arguments
///
/// * `input`: Input channel containing un-premultiplied alpha
/// * `alpha`: Alpha values for that channel
///
///
/// Array is modified in place
pub fn premultiply_f32(input: &mut [f32], alpha: &[f32]) {
    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        *color *= al;
    });
}

fn unpremultiply_f32_scalar(input: &mut [f32], alpha: &[f32]) {
    input.iter_mut().zip(alpha).for_each(|(color, al)| {
        if *al == 0.0 {
            *color = 0.0;
        } else {
            // avoid div by zero
            *color /= *al;
        }
    });
}

/// Undo the effects of pre-multiplied alpha on the array
///
/// # Arguments
///
/// * `input`: Input data
/// * `alpha`: Alpha associated for the input data
///
/// # Behaviour
/// - When alpha channel is zero, input also becomes zero
///
/// Array is modified in place
pub fn unpremultiply_f32(input: &mut [f32], alpha: &[f32]) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "portable-simd")]
        {
            use crate::premul_alpha::std_simd::unpremultiply_std_simd;
            unpremultiply_std_simd(input, alpha);
        }
    }
    unpremultiply_f32_scalar(input, alpha);
}
