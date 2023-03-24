#![cfg(feature = "jpeg")]
//! This represents a jpeg decoder instance
//!
//!
//! Re-exports all items in zune_jpeg library
//! and implements `DecoderTrait` for the library
//!
//!
use jpeg_encoder::{ColorType, EncodingError, JpegColorType};
use log::info;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_jpeg::errors::DecodeErrors;
/// Re-expose jpeg crate here
pub use zune_jpeg::{ImageInfo, JpegDecoder};

use crate::codecs::ImageFormat;
use crate::deinterleave::deinterleave_u8;
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::impls::depth::Depth;
use crate::metadata::ImageMetadata;
use crate::traits::{DecoderTrait, EncoderTrait, OperationsTrait};

impl<'a> DecoderTrait<'a> for zune_jpeg::JpegDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, crate::errors::ImageErrors>
    {
        let pixels = self
            .decode()
            .map_err(<DecodeErrors as Into<ImageErrors>>::into)?;

        let colorspace = self.get_out_colorspace();
        let (width, height) = self.get_dimensions().unwrap();

        let mut image = Image::from_u8(&pixels, width, height, colorspace);
        image.metadata.format = Some(ImageFormat::JPEG);

        Ok(image)
    }

    fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        self.dimensions()
            .map(|dims| (usize::from(dims.0), usize::from(dims.1)))
    }

    fn get_out_colorspace(&self) -> ColorSpace
    {
        self.get_output_colorspace().unwrap()
    }

    fn get_name(&self) -> &'static str
    {
        "JPEG decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors>
    {
        self.decode_headers()
            .map_err(<DecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.get_dimensions().unwrap();

        let metadata = ImageMetadata {
            format:        Some(ImageFormat::JPEG),
            colorspace:    self.get_input_colorspace().unwrap(),
            depth:         BitDepth::Eight,
            width:         width,
            height:        height,
            color_trc:     None,
            default_gamma: None
        };

        Ok(Some(metadata))
    }
}

impl From<zune_jpeg::errors::DecodeErrors> for ImageErrors
{
    fn from(from: zune_jpeg::errors::DecodeErrors) -> Self
    {
        let err = format!("jpg: {from:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}
// Okay I just need to really appreciate jpeg-encoder crate
// it's well written and covers quite a lot of things really
// well, so thank you Volker Ströbel

/// A simple JPEG encoder
#[derive(Copy, Clone, Default)]
pub struct JpegEncoder
{
    quality:           u8,
    progressive:       bool,
    optimized_huffman: bool
}

impl JpegEncoder
{
    /// Create a new decoder with a specified quality
    pub fn new(quality: u8) -> JpegEncoder
    {
        JpegEncoder {
            quality:           quality,
            progressive:       false,
            optimized_huffman: false
        }
    }
}

impl EncoderTrait for JpegEncoder
{
    fn get_name(&self) -> &'static str
    {
        "jpeg-encoder(vstroebel)"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImageErrors>
    {
        assert_eq!(
            image.get_depth(),
            BitDepth::Eight,
            "Unsupported bit depth{:?}",
            image.get_depth()
        );
        let pixels = &image.flatten_frames::<u8>()[0];

        if let Some(colorspace) = match_colorspace_to_colortype(image.get_colorspace())
        {
            let max_dims = usize::from(u16::MAX);

            let (width, height) = image.get_dimensions();

            // check dimensions
            if (width > max_dims) || (height > max_dims)
            {
                let msg = format!(
                    "Too large image dimensions {} x {}, maximum is {} x {}",
                    width, height, max_dims, max_dims
                );
                return Err(ImgEncodeErrors::ImageEncodeErrors(msg).into());
            }
            // create space for our encoder
            let mut encoded_data =
                Vec::with_capacity(width * height * image.get_colorspace().num_components());
            // create encoder finally
            // vec<u8> supports write so we use that as our encoder
            let mut encoder = jpeg_encoder::Encoder::new(&mut encoded_data, self.quality);

            // add options
            encoder.set_progressive(self.progressive);
            encoder.set_optimized_huffman_tables(self.optimized_huffman);

            encoder.encode(pixels, width as u16, height as u16, colorspace)?;

            Ok(encoded_data)
        }
        else
        {
            return Err(ImgEncodeErrors::UnsupportedColorspace(
                image.get_colorspace(),
                self.supported_colorspaces()
            )
            .into());
        }
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        // should match with the
        // jpeg-encoder crate
        &[
            ColorSpace::Luma,
            ColorSpace::RGB,
            ColorSpace::RGBA,
            ColorSpace::YCbCr,
            ColorSpace::YCCK,
            ColorSpace::CMYK
        ]
    }

    fn format(&self) -> ImageFormat
    {
        ImageFormat::JPEG
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth]
    {
        &[BitDepth::Eight]
    }

    fn default_depth(&self) -> BitDepth
    {
        BitDepth::Eight
    }
}

/// Match the library colorspace to jpeg color type
const fn match_colorspace_to_colortype(colorspace: ColorSpace) -> Option<ColorType>
{
    match colorspace
    {
        ColorSpace::RGBA => Some(ColorType::Rgba),
        ColorSpace::RGB => Some(ColorType::Rgb),
        ColorSpace::YCbCr => Some(ColorType::Ycbcr),
        ColorSpace::Luma => Some(ColorType::Luma),
        ColorSpace::YCCK => Some(ColorType::Ycck),
        ColorSpace::CMYK => Some(ColorType::Cmyk),
        _ => None
    }
}

impl From<EncodingError> for ImageErrors
{
    fn from(value: EncodingError) -> Self
    {
        ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(value.to_string()))
    }
}
