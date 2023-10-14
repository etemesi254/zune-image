/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Change image from pre-multiplied alpha to
//! un-premultiplied alpha and vice versa
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::log::warn;
use zune_imageprocs::premul_alpha::{
    create_unpremul_table_u16, create_unpremul_table_u8, premultiply_f32, premultiply_u16,
    premultiply_u8, unpremultiply_f32, unpremultiply_u16, unpremultiply_u8
};

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::metadata::AlphaState;
use crate::traits::OperationsTrait;

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
    pub fn new(to: AlphaState) -> PremultiplyAlpha {
        PremultiplyAlpha { to }
    }
}

impl OperationsTrait for PremultiplyAlpha {
    fn get_name(&self) -> &'static str {
        "pre-multiply alpha"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        if !image.get_colorspace().has_alpha() {
            warn!("Image colorspace indicates no alpha channel, this operation is a no-op");
            return Ok(());
        }

        let colorspaces = image.get_colorspace();
        let alpha_state = image.metadata.alpha;

        if alpha_state == self.to {
            warn!("Alpha is already in required mode, exiting");
            return Ok(());
        }

        let bit_type = image.get_depth();

        for image_frame in image.get_frames_mut() {
            // read colorspace
            // split between alpha and color channels
            let (color_channels, alpha) = image_frame
                .channels
                .split_at_mut(colorspaces.num_components() - 1);

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
                                self.get_name(),
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
                                self.get_name(),
                                d.bit_type()
                            ))
                        }
                    },
                    (_, _) => return Err(ImageErrors::GenericStr("Could not pre-multiply alpha"))
                }
            }
        }

        // update metadata
        image.metadata.alpha = self.to;

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::F32, BitType::U16, BitType::U8]
    }
}
