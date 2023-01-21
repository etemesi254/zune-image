#![cfg(feature = "png")]
//! Represents an png image decoder
use log::debug;
use zune_core::colorspace::ColorSpace;
use zune_core::result::DecodingResult;
use zune_png::error::PngErrors;
pub use zune_png::PngDecoder;

use crate::deinterleave::{deinterleave_u16, deinterleave_u8};
use crate::errors::ImgErrors;
use crate::image::Image;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for PngDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImgErrors>
    {
        let pixels = self
            .decode()
            .map_err(<PngErrors as Into<ImgErrors>>::into)?;

        let depth = self.get_depth().unwrap();
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        debug!("De-Interleaving image channel");

        let channel = match pixels
        {
            DecodingResult::U8(data) => deinterleave_u8(&data, colorspace).unwrap(),
            DecodingResult::U16(data) => deinterleave_u16(&data, colorspace).unwrap()
        };

        Ok(Image::new(channel, depth, width, height, colorspace))
    }

    fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        self.get_dimensions()
    }

    fn get_out_colorspace(&self) -> ColorSpace
    {
        self.get_colorspace().unwrap()
    }

    fn get_name(&self) -> &'static str
    {
        "Png Decoder"
    }
}

impl From<zune_png::error::PngErrors> for ImgErrors
{
    fn from(from: zune_png::error::PngErrors) -> Self
    {
        let err = format!("png: {from:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}
