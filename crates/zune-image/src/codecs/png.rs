#![cfg(feature = "png")]
//! Represents an png image decoder
use log::{debug, info};
use zune_core::bit_depth::BitDepth;
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

        let mut image = match pixels
        {
            DecodingResult::U8(data) => Image::from_u8(&data, width, height, colorspace),
            DecodingResult::U16(data) =>
            {
                Image::from_u16(&data, width, height, BitDepth::Sixteen, colorspace)
            }
        };

        // set gamma value or 2.2 if image has none.
        let gamma = self.get_gamma().unwrap_or(1.0 / 2.2);
        info!("Setting gama value to be {}", gamma);
        image.set_default_gamma(gamma);
        Ok(image)
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
