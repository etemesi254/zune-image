#![cfg(feature = "png")]
//! Represents an png image decoder
use log::{debug, info};
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_core::result::DecodingResult;
use zune_png::error::PngErrors;
pub use zune_png::PngDecoder;

use crate::codecs::ImageFormat;
use crate::deinterleave::{deinterleave_u16, deinterleave_u8};
use crate::errors::ImgErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for PngDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImgErrors>
    {
        let pixels = self
            .decode()
            .map_err(<PngErrors as Into<ImgErrors>>::into)?;

        let depth = self.get_depth().unwrap();
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        debug!("De-Interleaving image channel");

        let mut image = match pixels
        {
            DecodingResult::U8(data) => Image::from_u8(&data, width, height, colorspace),
            DecodingResult::U16(data) =>
            {
                Image::from_u16(&data, width, height, BitDepth::Sixteen, colorspace)
            }
            _ => unreachable!()
        };

        // set metadata details
        image.metadata.format = Some(ImageFormat::PNG);
        image.metadata.default_gamma = self.get_gamma();

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

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImgErrors>
    {
        self.decode_headers()
            .map_err(<PngErrors as Into<ImgErrors>>::into)?;

        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_depth().unwrap();

        let metadata = ImageMetadata {
            format:        Some(ImageFormat::PNG),
            colorspace:    self.get_colorspace().unwrap(),
            depth:         depth,
            width:         width,
            height:        height,
            color_trc:     None,
            default_gamma: self.get_gamma()
        };

        Ok(Some(metadata))
    }
}

impl From<zune_png::error::PngErrors> for ImgErrors
{
    fn from(from: zune_png::error::PngErrors) -> Self
    {
        let err = format!("png: {from:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}
