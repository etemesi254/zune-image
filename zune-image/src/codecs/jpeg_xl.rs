#![cfg(feature = "jpeg-xl")]
//! A simple jxl lossless encoder
//!
//! The encoder supports simple lossless image
//! (modular, no var-dct) with support for 8 bit and
//! 16 bit images with no palette support
//!
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
pub use zune_jpegxl::*;

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::traits::EncoderTrait;

/// A simple JXL encoder that ties the bridge between
/// Image struct and the [zune_jpegxl::SimpleJxlEncoder](zune_jpegxl::JxlSimpleEncoder)
#[derive(Default, Copy, Clone)]
pub struct JxlEncoder
{
    options: Option<EncoderOptions>
}

impl JxlEncoder
{
    /// Create a new encoder with default options
    ///
    /// Default options include 4 threads for encoding,and an effort
    /// od 4
    pub fn new() -> JxlEncoder
    {
        JxlEncoder::default()
    }
    /// Create new encoder with custom options
    pub fn new_with_options(options: EncoderOptions) -> JxlEncoder
    {
        JxlEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for JxlEncoder
{
    fn get_name(&self) -> &'static str
    {
        "jxl-encoder"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImageErrors>
    {
        let options = create_options_for_encoder(self.options, image);

        let data = &image.to_u8()[0];

        let encoder = JxlSimpleEncoder::new(data, options);

        let data = encoder
            .encode()
            .map_err(<JxlEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::Luma,
            ColorSpace::LumaA,
            ColorSpace::RGBA,
            ColorSpace::RGB
        ]
    }

    fn format(&self) -> ImageFormat
    {
        ImageFormat::JPEG_XL
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth]
    {
        &[BitDepth::Eight, BitDepth::Sixteen]
    }

    fn default_depth(&self, depth: BitDepth) -> BitDepth
    {
        match depth
        {
            BitDepth::Sixteen | BitDepth::Float32 => BitDepth::Sixteen,
            _ => BitDepth::Eight
        }
    }

    fn set_options(&mut self, options: EncoderOptions)
    {
        self.options = Some(options)
    }
}

impl From<JxlEncodeErrors> for ImgEncodeErrors
{
    fn from(value: JxlEncodeErrors) -> Self
    {
        ImgEncodeErrors::ImageEncodeErrors(format!("{:?}", value))
    }
}
