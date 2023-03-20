#![cfg(feature = "farbfeld")]

use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
pub use zune_farbfeld::*;

use crate::codecs::ImageFormat;
use crate::deinterleave::deinterleave_u16;
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecoderTrait, EncoderTrait};

impl<'a> DecoderTrait<'a> for FarbFeldDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImageErrors>
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

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors>
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

pub struct FarbFeldEncoder;

impl EncoderTrait for FarbFeldEncoder
{
    fn get_name(&self) -> &'static str
    {
        "farbfeld"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImageErrors>
    {
        let (width, height) = image.get_dimensions();
        let options = EncoderOptions::default()
            .set_colorspace(image.get_colorspace())
            .set_width(width)
            .set_height(height)
            .set_depth(image.get_depth());
        let data = image.to_u8();

        let encoder_options = zune_farbfeld::FarbFeldEncoder::new(&data, options);

        let data = encoder_options
            .encode()
            .map_err(<FarbFeldEncoderErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[ColorSpace::RGBA]
    }

    fn format(&self) -> ImageFormat
    {
        ImageFormat::Farbfeld
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth]
    {
        &[BitDepth::Sixteen]
    }

    fn default_depth(&self) -> BitDepth
    {
        BitDepth::Sixteen
    }

    fn default_colorspace(&self, _: ColorSpace) -> ColorSpace
    {
        ColorSpace::RGBA
    }
}

impl From<FarbFeldEncoderErrors> for ImgEncodeErrors
{
    fn from(value: FarbFeldEncoderErrors) -> Self
    {
        ImgEncodeErrors::ImageEncodeErrors(format!("{:?}", value))
    }
}
