/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Colorspace conversion routines
//!
//!
//! This contains simple colorspace conversion routines
//! that convert between different colorspaces in images
//!
//! ## Intermediate conversions
//! Some colorspaces do not have a 1 to 1 mapping e.g CMYK to HSL, for such colorspaces
//! the library converts the image to an intermediate colorspace(most of the time this is RGB or RGBA)
//! and then from the intermediate color converts it to the desired colorspace
//!
use zune_core::bit_depth::BitType;
use zune_core::colorspace::{ColorSpace, ALL_COLORSPACES};

use crate::core_filters::colorspace::conversion_functions::{
    convert_adding_opaque_alpha, convert_cmyk_to_rgb, convert_hsl_to_rgb, convert_hsv_to_rgb,
    convert_luma_to_rgb, convert_rgb_bgr, convert_rgb_to_argb, convert_rgb_to_cmyk,
    convert_rgb_to_grayscale, convert_rgb_to_hsl, convert_rgb_to_hsv,
    convert_rgba_to_argb_or_vice_versa, pop_channel
};
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

mod grayscale;
//mod rgb_to_hsl;
mod rgb_to_xyb;

mod conversion_functions;
mod rgb_to_cmyk;
mod rgb_to_hsl;
mod rgb_to_hsv;
mod tests;

/// Colorspace conversion filter
///
/// This filter allows one to convert from a colorspace to another, while preserving
/// bit depth, there exists multiple mappings with different colorspace
///
///
/// This filter can also be accessed via
/// [`image.convert_color()`](crate::image::Image::convert_color)
pub struct ColorspaceConv {
    to: ColorSpace
}

impl ColorspaceConv {
    pub fn new(to: ColorSpace) -> ColorspaceConv {
        ColorspaceConv { to }
    }
}
impl OperationsTrait for ColorspaceConv {
    fn name(&self) -> &'static str {
        "Colorspace conversion"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let from = image.colorspace();

        // colorspace matches
        if from == self.to {
            return Ok(());
        }

        match from {
            ColorSpace::RGB => match self.to {
                ColorSpace::RGBA => convert_adding_opaque_alpha(image)?,
                ColorSpace::Luma => convert_rgb_to_grayscale(image, self.to, self.to.has_alpha())?,
                ColorSpace::LumaA => convert_rgb_to_grayscale(image, self.to, self.to.has_alpha())?,
                ColorSpace::CMYK => convert_rgb_to_cmyk(image)?,
                ColorSpace::BGR => convert_rgb_bgr(from, self.to, image)?,
                ColorSpace::BGRA => convert_rgb_bgr(from, self.to, image)?,
                ColorSpace::ARGB => convert_rgb_to_argb(image)?,
                ColorSpace::HSL => convert_rgb_to_hsl(image)?,
                ColorSpace::HSV => convert_rgb_to_hsv(image)?,
                color => {
                    let msg = format!("Unsupported/unknown mapping from RGB to {color:?}");
                    return Err(ImageErrors::GenericString(msg));
                }
            },
            ColorSpace::RGBA => match self.to {
                ColorSpace::RGB => pop_channel(image),
                ColorSpace::BGR => convert_rgb_bgr(from, self.to, image)?,
                ColorSpace::BGRA => convert_rgb_bgr(from, self.to, image)?,
                ColorSpace::ARGB => convert_rgba_to_argb_or_vice_versa(image)?,
                ColorSpace::LumaA => convert_rgb_to_grayscale(image, self.to, self.to.has_alpha())?,
                ColorSpace::Luma => convert_rgb_to_grayscale(image, self.to, self.to.has_alpha())?,
                ColorSpace::HSV => convert_rgb_to_hsv(image)?,
                ColorSpace::HSL => convert_rgb_to_hsl(image)?,
                ColorSpace::CMYK => {
                    // drop alpha
                    pop_channel(image);
                    convert_rgb_to_cmyk(image)?;
                }
                color => {
                    let msg = format!("Unsupported/unknown mapping from RGBA to {color:?}");
                    return Err(ImageErrors::GenericString(msg));
                }
            },
            ColorSpace::Luma => match self.to {
                ColorSpace::RGB => convert_luma_to_rgb(image, self.to)?,
                ColorSpace::LumaA => convert_adding_opaque_alpha(image)?,
                color => {
                    // convert to rgb
                    convert_luma_to_rgb(image, ColorSpace::RGB)?;
                    image.set_colorspace(ColorSpace::RGB);
                    image.convert_color(color)?;
                }
            },

            ColorSpace::LumaA => match self.to {
                ColorSpace::RGB => convert_luma_to_rgb(image, self.to)?,
                ColorSpace::RGBA => convert_luma_to_rgb(image, self.to)?,
                ColorSpace::Luma => pop_channel(image),
                color => {
                    // convert to rgba
                    convert_luma_to_rgb(image, ColorSpace::RGBA)?;
                    image.set_colorspace(ColorSpace::RGBA);
                    image.convert_color(color)?;
                }
            },
            ColorSpace::CMYK => {
                // convert to RGB first
                convert_cmyk_to_rgb(image, ColorSpace::RGB)?;
                image.set_colorspace(ColorSpace::RGB);
                // convert to desired colorspace
                image.convert_color(self.to)?;
            }
            ColorSpace::BGR => {
                // first convert to rgb
                convert_rgb_bgr(from, ColorSpace::RGB, image)?;
                // then convert to desired color
                image.set_colorspace(ColorSpace::RGB);
                image.convert_color(self.to)?;
            }
            ColorSpace::BGRA => {
                // BGRA and RGBA are similar with difference being only the R and B are swapped
                // so convert to RGBA
                convert_rgb_bgr(image.colorspace(), ColorSpace::RGBA, image)?;

                // then use RGBA conversions
                image.set_colorspace(ColorSpace::RGBA);
                image.convert_color(self.to)?;
            }

            ColorSpace::ARGB => {
                // convert to RGBA
                convert_rgba_to_argb_or_vice_versa(image)?;
                image.set_colorspace(ColorSpace::RGBA);
                image.convert_color(self.to)?;
            }
            ColorSpace::HSL => {
                // convert to rgb
                convert_hsl_to_rgb(image)?;
                image.set_colorspace(ColorSpace::RGB);
                // convert to desired colorspace
                image.convert_color(self.to)?;
            }
            ColorSpace::HSV => {
                // convert to rgb
                convert_hsv_to_rgb(image)?;
                image.set_colorspace(ColorSpace::RGB);
                // convert to desired colorspace
                image.convert_color(self.to)?;
            }

            color => {
                let msg = format!("Unsupported colorspace  {color:?}");
                return Err(ImageErrors::GenericString(msg));
            }
        }
        // set it to the new colorspace
        image.set_colorspace(self.to);

        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &ALL_COLORSPACES
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U16, BitType::U8, BitType::F32]
    }
}
