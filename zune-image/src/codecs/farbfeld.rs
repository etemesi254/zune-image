#![cfg(feature = "farbfeld")]

use zune_core::colorspace::ColorSpace;
pub use zune_farbfeld::*;

use crate::deinterleave::deinterleave_u16;
use crate::errors::ImgErrors;
use crate::image::Image;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for FarbFeldDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImgErrors>
    {
        let pixels = self.decode()?;
        let colorspace = self.get_colorspace();
        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_bit_depth();
        let channels = deinterleave_u16(&pixels, colorspace)?;

        Ok(Image::new(channels, depth, width, height, colorspace))
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
        "Farbfeld Decoder"
    }

    fn is_experimental(&self) -> bool
    {
        true
    }
}
