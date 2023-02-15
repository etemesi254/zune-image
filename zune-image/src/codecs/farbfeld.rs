#![cfg(feature = "farbfeld")]

use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
pub use zune_farbfeld::*;

use crate::codecs::ImageFormat;
use crate::deinterleave::deinterleave_u16;
use crate::errors::ImgErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for FarbFeldDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImgErrors>
    {
        let pixels = self.decode()?;
        let colorspace = self.get_colorspace();
        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_bit_depth();

        let mut image = Image::from_u16(&pixels, width, height, depth, colorspace);

        image.metadata.format = Some(ImageFormat::Farbfeld);

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
        "Farbfeld Decoder"
    }

    fn is_experimental(&self) -> bool
    {
        true
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImgErrors>
    {
        self.decode_headers()?;

        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_bit_depth();

        let metadata = ImageMetadata {
            format:        Some(ImageFormat::PNG),
            colorspace:    self.get_colorspace(),
            depth:         depth,
            width:         width,
            height:        height,
            color_trc:     None,
            default_gamma: None
        };

        Ok(Some(metadata))
    }
}
