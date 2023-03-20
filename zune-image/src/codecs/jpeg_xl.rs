#![cfg(feature = "jpeg-xl")]

use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_jpegxl::{JxlEncodeErrors, JxlSimpleEncoder};

use crate::codecs::ImageFormat;
use crate::errors::{ImgEncodeErrors, ImgErrors};
use crate::image::Image;
use crate::traits::EncoderTrait;

pub struct JxlEncoder;

impl EncoderTrait for JxlEncoder
{
    fn get_name(&self) -> &'static str
    {
        "jxl-encoder"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImgErrors>
    {
        let (width, height) = image.get_dimensions();
        let colorspace = image.get_colorspace();
        let depth = image.get_depth();

        let options = EncoderOptions::default()
            .set_width(width)
            .set_height(height)
            .set_colorspace(colorspace)
            .set_depth(depth);

        let data = image.to_u8();

        let mut encoder = JxlSimpleEncoder::new(&data, options);

        let data = encoder
            .encode()
            .map_err(<JxlEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::Luma,
            ColorSpace::LumaA,
            ColorSpace::RGBA,
            ColorSpace::RGB
        ]
    }

    fn format(&self) -> ImageFormat
    {
        ImageFormat::JPEG_XL
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth]
    {
        &[
            BitDepth::Eight,
            BitDepth::Ten,
            BitDepth::Twelve,
            BitDepth::Sixteen
        ]
    }

    fn default_depth(&self) -> BitDepth
    {
        BitDepth::Eight
    }
}

impl From<JxlEncodeErrors> for ImgEncodeErrors
{
    fn from(value: JxlEncodeErrors) -> Self
    {
        ImgEncodeErrors::ImageEncodeErrors(format!("{:?}", value))
    }
}
