#![cfg(feature = "zune-jpeg")]
//! This represents a jpeg decoder instance
//!
//!
//! Re-exports all items in zune_jpeg library
//! and implements `DecoderTrait` for the library
//!
//!
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_jpeg::errors::DecodeErrors;
/// Re-expose jpeg crate here
pub use zune_jpeg::*;

use crate::codecs::ImageFormat;
use crate::deinterleave::deinterleave_u8;
use crate::errors::ImgErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for zune_jpeg::JpegDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, crate::errors::ImgErrors>
    {
        let pixels = self
            .decode()
            .map_err(<DecodeErrors as Into<ImgErrors>>::into)?;

        let colorspace = self.get_out_colorspace();
        let (width, height) = self.get_dimensions().unwrap();

        let mut image = Image::from_u8(&pixels, width, height, colorspace);
        image.metadata.format = Some(ImageFormat::JPEG);

        Ok(image)
    }

    fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        let width = usize::from(self.width());
        let height = usize::from(self.height());

        Some((width, height))
    }

    fn get_out_colorspace(&self) -> ColorSpace
    {
        self.get_output_colorspace()
    }

    fn get_name(&self) -> &'static str
    {
        "JPEG decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImgErrors>
    {
        self.decode_headers()
            .map_err(<DecodeErrors as Into<ImgErrors>>::into)?;

        let (width, height) = self.get_dimensions().unwrap();

        let metadata = ImageMetadata {
            format:        Some(ImageFormat::JPEG),
            colorspace:    self.get_output_colorspace(),
            depth:         BitDepth::Eight,
            width:         width,
            height:        height,
            color_trc:     None,
            default_gamma: None
        };

        Ok(Some(metadata))
    }
}

impl From<zune_jpeg::errors::DecodeErrors> for ImgErrors
{
    fn from(from: zune_jpeg::errors::DecodeErrors) -> Self
    {
        let err = format!("jpg: {from:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}
