#![cfg(feature = "psd")]

use log::debug;
use zune_core::colorspace::ColorSpace;
use zune_core::result::DecodingResult;
pub use zune_psd::PSDDecoder;

use crate::deinterleave::{deinterleave_u16, deinterleave_u8};
use crate::errors::ImgErrors;
use crate::image::Image;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for PSDDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImgErrors>
    {
        let pixels = self.decode()?;

        let depth = self.get_bit_depth().unwrap();
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace();

        debug!("De-Interleaving image channel");

        let image = match pixels
        {
            DecodingResult::U8(data) => Image::from_u8(&data, width, height, colorspace),
            DecodingResult::U16(data) => Image::from_u16(&data, width, height, depth, colorspace)
        };

        Ok(image)
    }

    fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        self.get_dimensions()
    }

    fn get_out_colorspace(&self) -> ColorSpace
    {
        self.get_colorspace()
    }

    fn get_name(&self) -> &'static str
    {
        "PSD Decoder"
    }

    fn is_experimental(&self) -> bool
    {
        true
    }
}

impl From<zune_psd::errors::PSDDecodeErrors> for ImgErrors
{
    fn from(error: zune_psd::errors::PSDDecodeErrors) -> Self
    {
        let err = format!("psd: {error:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}
