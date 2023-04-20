#![cfg(feature = "hdr")]
//! Radiance HDR decoding support
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
pub use zune_hdr::*;

use crate::codecs::ImageFormat;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for HdrDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImageErrors>
    {
        let bytes = self.decode()?;
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        Ok(Image::from_f32(&bytes, width, height, colorspace))
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
        "HDR decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, ImageErrors>
    {
        self.decode_headers()?;

        let (width, height) = self.get_dimensions().unwrap();

        let metadata = ImageMetadata {
            width: width,
            height: height,
            colorspace: ColorSpace::RGB,
            depth: BitDepth::Float32,
            format: Some(ImageFormat::HDR),
            ..Default::default()
        };
        Ok(Some(metadata))
    }
}

impl From<HdrDecodeErrors> for ImageErrors
{
    fn from(value: HdrDecodeErrors) -> Self
    {
        Self::ImageDecodeErrors(format!("hdr: {value:?}"))
    }
}
