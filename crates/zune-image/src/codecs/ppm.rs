#![cfg(feature = "ppm")]
//! Represents a PPM and PAL image encoder
use log::debug;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_core::result::DecodingResult;
pub use zune_ppm::PPMDecoder;
use zune_ppm::{PPMDecodeErrors, PPMEncodeErrors, PPMEncoder as PPMEnc};

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::deinterleave::{deinterleave_u16, deinterleave_u8};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecoderTrait, EncoderTrait};

#[derive(Copy, Clone, Default)]
pub struct PPMEncoder
{
    options: Option<EncoderOptions>
}

impl PPMEncoder
{
    pub fn new() -> PPMEncoder
    {
        PPMEncoder { options: None }
    }
    pub fn new_with_options(options: EncoderOptions) -> PPMEncoder
    {
        PPMEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for PPMEncoder
{
    fn get_name(&self) -> &'static str
    {
        "PPM Encoder"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImageErrors>
    {
        let options = create_options_for_encoder(self.options, image);

        let data = &image.to_u8()[0];

        let ppm_encoder = PPMEnc::new(data, options);

        let data = ppm_encoder
            .encode()
            .map_err(<PPMEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGB,  // p7
            ColorSpace::Luma, // p7
            ColorSpace::RGBA, // p7
            ColorSpace::LumaA
        ]
    }

    fn format(&self) -> ImageFormat
    {
        ImageFormat::PPM
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth]
    {
        &[BitDepth::Sixteen, BitDepth::Eight]
    }

    fn default_depth(&self) -> BitDepth
    {
        BitDepth::Eight
    }
}

impl<'a> DecoderTrait<'a> for PPMDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImageErrors>
    {
        let pixels = self.decode()?;

        let depth = self.get_bit_depth().unwrap();
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        let mut image = match pixels
        {
            DecodingResult::U8(data) => Image::from_u8(&data, width, height, colorspace),
            DecodingResult::U16(data) => Image::from_u16(&data, width, height, colorspace),
            _ => unreachable!()
        };

        // set metadata details
        image.metadata.format = Some(ImageFormat::PPM);

        Ok(image)
    }

    fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        self.get_dimensions()
    }

    fn get_out_colorspace(&self) -> ColorSpace
    {
        self.get_colorspace().unwrap_or(ColorSpace::Unknown)
    }

    fn get_name(&self) -> &'static str
    {
        "PPM Decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors>
    {
        self.read_headers()
            .map_err(<PPMDecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_bit_depth().unwrap();

        let metadata = ImageMetadata {
            format: Some(ImageFormat::PPM),
            colorspace: self.get_colorspace().unwrap(),
            depth: depth,
            width: width,
            height: height,
            ..Default::default()
        };

        Ok(Some(metadata))
    }
}

#[cfg(feature = "ppm")]
impl From<zune_ppm::PPMDecodeErrors> for ImageErrors
{
    fn from(from: zune_ppm::PPMDecodeErrors) -> Self
    {
        let err = format!("ppm: {from:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}

#[cfg(feature = "ppm")]
impl From<zune_ppm::PPMEncodeErrors> for ImgEncodeErrors
{
    fn from(error: zune_ppm::PPMEncodeErrors) -> Self
    {
        let err = format!("ppm: {error:?}");

        ImgEncodeErrors::ImageEncodeErrors(err)
    }
}
