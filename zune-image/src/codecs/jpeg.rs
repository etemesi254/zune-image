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

use crate::deinterleave::deinterleave_u8;
use crate::errors::ImgErrors;
use crate::image::Image;
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

        let image = Image::from_u8(&pixels, width, height, colorspace);

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
        "Jpeg decoder"
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
