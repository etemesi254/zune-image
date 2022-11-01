#![cfg(feature = "png")]

use zune_core::colorspace::ColorSpace;
use zune_png::error::PngErrors;
pub use zune_png::PngDecoder;

use crate::errors::ImgErrors;
use crate::image::Image;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for PngDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImgErrors>
    {
        let _ = self
            .decode()
            .map_err(<PngErrors as Into<ImgErrors>>::into)?;

        Err(ImgErrors::GenericStr("Incomplete"))
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
        "Png Decoder"
    }
}
