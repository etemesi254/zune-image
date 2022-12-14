#![cfg(feature = "qoi")]

use zune_core::colorspace::ColorSpace;
pub use zune_qoi::*;

use crate::deinterleave::deinterleave_u8;
use crate::errors::ImgErrors;
use crate::image::Image;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for QoiDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImgErrors>
    {
        let pixels = self.decode()?;
        // safe because these are none when we haven't decoded.
        let colorspace = self.get_colorspace().unwrap();
        let (width, height) = self.get_dimensions().unwrap();

        let depth = self.get_bit_depth();
        let channels = deinterleave_u8(&pixels, colorspace)?;

        Ok(Image::new(channels, depth, width, height, colorspace))
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
        "Farbfeld Decoder"
    }

    fn is_experimental(&self) -> bool
    {
        true
    }
}
