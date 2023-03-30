#![cfg(feature = "qoi")]

use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
pub use zune_qoi::*;

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::deinterleave::deinterleave_u8;
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecodeInto, DecoderTrait, EncoderTrait};

impl<'a> DecoderTrait<'a> for QoiDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImageErrors>
    {
        let pixels = self.decode()?;
        // safe because these are none when we haven't decoded.
        let colorspace = self.get_colorspace().unwrap();
        let (width, height) = self.get_dimensions().unwrap();

        let depth = self.get_bit_depth();

        let mut image = Image::from_u8(&pixels, width, height, colorspace);

        // set metadata details
        image.metadata.format = Some(ImageFormat::QOI);

        Ok(image)
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
        "QOI Decoder"
    }

    fn is_experimental(&self) -> bool
    {
        true
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors>
    {
        self.decode_headers()
            .map_err(<QoiErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_bit_depth();

        let metadata = ImageMetadata {
            format: Some(ImageFormat::QOI),
            colorspace: self.get_colorspace().unwrap(),
            depth: depth,
            width: width,
            height: height,
            ..Default::default()
        };

        Ok(Some(metadata))
    }
}

#[derive(Copy, Clone, Default)]
pub struct QoiEncoder
{
    options: Option<EncoderOptions>
}

impl QoiEncoder
{
    pub fn new() -> QoiEncoder
    {
        QoiEncoder::default()
    }

    pub fn new_with_options(options: EncoderOptions) -> QoiEncoder
    {
        QoiEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for QoiEncoder
{
    fn get_name(&self) -> &'static str
    {
        "QOI Encoder"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImageErrors>
    {
        let options = create_options_for_encoder(self.options, image);

        let data = &image.to_u8()[0];

        let mut qoi_encoder = zune_qoi::QoiEncoder::new(data, options);

        let data = qoi_encoder
            .encode()
            .map_err(<QoiEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[ColorSpace::RGBA, ColorSpace::RGB]
    }

    fn format(&self) -> ImageFormat
    {
        ImageFormat::QOI
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth]
    {
        &[BitDepth::Eight]
    }

    fn default_depth(&self) -> BitDepth
    {
        BitDepth::Eight
    }

    fn default_colorspace(&self, colorspace: ColorSpace) -> ColorSpace
    {
        // if colorspace has an alpha channel,
        // we want to preserve it in the final encoder
        if colorspace.has_alpha()
        {
            ColorSpace::RGBA
        }
        else
        {
            // otherwise, just stick up to the one we know
            ColorSpace::RGB
        }
    }
}

impl From<zune_qoi::QoiErrors> for ImageErrors
{
    fn from(error: zune_qoi::QoiErrors) -> Self
    {
        let err = format!("qoi: {error:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}

impl From<zune_qoi::QoiEncodeErrors> for ImgEncodeErrors
{
    fn from(error: zune_qoi::QoiEncodeErrors) -> Self
    {
        let err = format!("qoi: {error:?}");

        ImgEncodeErrors::Generic(err)
    }
}

impl<'b> DecodeInto for QoiDecoder<'b>
{
    fn decode_into(&mut self, buffer: &mut [u8]) -> Result<(), ImageErrors>
    {
        self.decode_into(buffer)?;

        Ok(())
    }

    fn output_buffer_size(&mut self) -> Result<usize, ImageErrors>
    {
        self.decode_headers()?;

        // unwrap is okay because we successfully decoded image headers
        Ok(self.output_buffer_size().unwrap())
    }
}
