#![cfg(feature = "png")]
#![allow(unused_variables)]

//! Represents an png image decoder
use exif::experimental::Writer;
use log::{debug, info};
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_core::result::DecodingResult;
pub use zune_png::*;

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::deinterleave::{deinterleave_u16, deinterleave_u8};
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecoderTrait, EncoderTrait};

impl<'a> DecoderTrait<'a> for PngDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImageErrors>
    {
        let metadata = self.read_headers()?.unwrap();

        let pixels = self
            .decode()
            .map_err(<error::PngDecodeErrors as Into<ImageErrors>>::into)?;

        let depth = self.get_depth().unwrap();
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        let mut image = match pixels
        {
            DecodingResult::U8(data) => Image::from_u8(&data, width, height, colorspace),
            DecodingResult::U16(data) => Image::from_u16(&data, width, height, colorspace),
            _ => unreachable!()
        };
        // metadata
        image.metadata = metadata;

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
        "PNG Decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors>
    {
        self.decode_headers()
            .map_err(<error::PngDecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_depth().unwrap();

        let mut metadata = ImageMetadata {
            format: Some(ImageFormat::PNG),
            colorspace: self.get_colorspace().unwrap(),
            depth: depth,
            width: width,
            height: height,
            default_gamma: self.get_info().unwrap().gamma,
            ..Default::default()
        };
        #[cfg(feature = "metadata")]
        {
            let info = self.get_info().unwrap();
            // see if we have an exif chunk
            if let Some(exif) = info.exif
            {
                metadata.parse_raw_exif(exif)
            }
        }

        Ok(Some(metadata))
    }
}

impl From<zune_png::error::PngDecodeErrors> for ImageErrors
{
    fn from(from: zune_png::error::PngDecodeErrors) -> Self
    {
        let err = format!("png: {from:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}

#[derive(Default)]
pub struct PngEncoder
{
    options: Option<EncoderOptions>
}

impl PngEncoder
{
    pub fn new() -> PngEncoder
    {
        PngEncoder::default()
    }
    pub fn new_with_options(options: EncoderOptions) -> PngEncoder
    {
        PngEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for PngEncoder
{
    fn get_name(&self) -> &'static str
    {
        "PNG encoder"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImageErrors>
    {
        let options = create_options_for_encoder(self.options, image);

        let frame = &image.to_u8_be()[0];

        let mut encoder = zune_png::PngEncoder::new(frame, options);

        let mut buf = std::io::Cursor::new(vec![]);

        #[cfg(feature = "metadata")]
        {
            use exif::experimental::Writer;

            if !options.strip_metadata()
            {
                if let Some(fields) = &image.metadata.exif
                {
                    let mut writer = Writer::new();

                    for metadatum in fields
                    {
                        writer.push_field(metadatum);
                    }
                    let result = writer.write(&mut buf, false);
                    if result.is_ok()
                    {
                        encoder.add_exif_segment(buf.get_ref());
                    }
                    else
                    {
                        log::warn!("Writing exif failed {:?}", result);
                    }
                }
            }
        }
        Ok(encoder.encode())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::Luma,
            ColorSpace::LumaA,
            ColorSpace::RGB,
            ColorSpace::RGBA
        ]
    }

    fn format(&self) -> ImageFormat
    {
        ImageFormat::PNG
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth]
    {
        &[BitDepth::Eight, BitDepth::Sixteen]
    }

    fn default_depth(&self, depth: BitDepth) -> BitDepth
    {
        match depth
        {
            BitDepth::Sixteen | BitDepth::Float32 => BitDepth::Sixteen,
            _ => BitDepth::Eight
        }
    }
}
