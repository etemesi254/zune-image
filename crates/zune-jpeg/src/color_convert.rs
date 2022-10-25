#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::too_many_arguments,
    clippy::doc_markdown
)]

//! Color space conversion routines
//!
//! This files exposes functions to convert one colorspace to another in a jpeg
//! image
//!
//! Currently supported conversions are
//!
//! - `YCbCr` to `RGB,RGBA,GRAYSCALE,RGBX`.
//!
//!
//! Hey there, if your reading this it means you probably need something, so let me help you.
//!
//! There are 3 supported cpu extensions here.
//! 1. Scalar
//! 2. SSE
//! 3. AVX
//!
//! There are two types of the color convert functions
//!
//! 1. Acts on 16 pixels.
//! 2. Acts on 8 pixels.
//!
//! The reason for this is because when implementing the AVX part it occurred to me that we can actually
//! do better and process 2 MCU's if we change IDCT return type to be `i16's`, since a lot of
//! CPU's these days support AVX extensions, it becomes nice if we optimize for that path ,
//! therefore AVX routines can process 16 pixels directly and SSE and Scalar just compensate.
//!
//! By compensating, I mean I wrote the 16 pixels version operating on the 8 pixel version twice.
//!
//! Therefore if your looking to optimize some routines, probably start there.

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(feature = "x86")]
pub use crate::color_convert::avx::{ycbcr_to_rgb_avx2, ycbcr_to_rgba_avx2, ycbcr_to_rgbx_avx2};
#[cfg(feature = "x86")]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use crate::color_convert::sse::{ycbcr_to_rgb_sse, ycbcr_to_rgb_sse_16, ycbcr_to_rgba_sse_16};
use crate::decoder::ColorConvert16Ptr;

mod avx;
mod scalar;
mod sse;

pub use scalar::{ycbcr_to_grayscale, ycbcr_to_ycbcr};
use zune_core::colorspace::ColorSpace;

/// This function determines the best color-convert function to carry out
/// based on the colorspace needed

pub fn choose_ycbcr_to_rgb_convert_func(
    type_need: ColorSpace, use_unsafe: bool,
) -> Option<ColorConvert16Ptr>
{
    if use_unsafe
    {
        #[cfg(feature = "x86")]
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2")
            {
                debug!("Using AVX optimised color conversion functions");

                // I believe avx2 means sse4 is also available
                // match colorspace
                return match type_need
                {
                    ColorSpace::RGB => Some(ycbcr_to_rgb_avx2),
                    ColorSpace::RGBA => Some(ycbcr_to_rgba_avx2),
                    ColorSpace::YCbCr => Some(ycbcr_to_ycbcr),
                    ColorSpace::RGBX => Some(ycbcr_to_rgbx_avx2),
                    _ => None,
                };
            }
            // try sse
            else if is_x86_feature_detected!("sse4.1")
            {
                // I believe avx2 means sse4 is also available
                // match colorspace
                debug!("No support for avx2 switching to sse");
                debug!("Using sse color convert functions");
                return match type_need
                {
                    ColorSpace::RGB => Some(ycbcr_to_rgb_sse_16),
                    ColorSpace::RGBA | ColorSpace::RGBX => Some(ycbcr_to_rgba_sse_16),
                    ColorSpace::YCbCr => Some(ycbcr_to_ycbcr),
                    _ => None,
                };
            }
        }
    }
    // when there is no x86 or we haven't returned by here, resort to scalar
    return match type_need
    {
        ColorSpace::RGB => Some(scalar::ycbcr_to_rgb_16_scalar),
        ColorSpace::RGBA | ColorSpace::RGBX => Some(scalar::ycbcr_to_rgba_16_scalar),
        ColorSpace::YCbCr => Some(ycbcr_to_ycbcr),

        _ => None,
    };
}
