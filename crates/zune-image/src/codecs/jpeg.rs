#![cfg(feature = "zune-jpeg")]
use zune_core::colorspace::ColorSpace;
/// Re-expose jpeg crate here
pub use zune_jpeg::*;

use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for zune_jpeg::JpegDecoder<'a>
{
    fn decode_buffer(&mut self) -> Result<Vec<u8>, crate::errors::ImgErrors>
    {
        self.decode_buffer().map_err(|x| x.into())
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
