#![cfg(feature = "jpeg-xl")]

use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_jpegxl::{JxlEncodeErrors, JxlSimpleEncoder};

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::traits::EncoderTrait;

#[derive(Default, Copy, Clone)]
pub struct JxlEncoder
{
    options: Option<EncoderOptions>
}

impl JxlEncoder
{
    pub fn new() -> JxlEncoder
    {
        JxlEncoder::default()
    }
    pub fn new_with_options(options: EncoderOptions) -> JxlEncoder
    {
        JxlEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for JxlEncoder
{
    fn get_name(&self) -> &'static str
    {
        "jxl-encoder"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImageErrors>
    {
        let options = create_options_for_encoder(self.options, image);

        let data = &image.to_u8()[0];

        let encoder = JxlSimpleEncoder::new(data, options);

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
        &[BitDepth::Eight, BitDepth::Sixteen]
    }

    fn default_depth(&self) -> BitDepth
    {
        BitDepth::Eight
    }

    fn set_options(&mut self, options: EncoderOptions)
    {
        self.options = Some(options)
    }
}

impl From<JxlEncodeErrors> for ImgEncodeErrors
{
    fn from(value: JxlEncodeErrors) -> Self
    {
        ImgEncodeErrors::ImageEncodeErrors(format!("{:?}", value))
    }
}
