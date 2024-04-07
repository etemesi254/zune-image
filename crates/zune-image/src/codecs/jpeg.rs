/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(feature = "jpeg")]
//! Jpeg decoding and encoding support
//!
//! The decoder uses a delegate library [`zune-jpeg`](zune_jpeg)
//! for decoding and [`jpeg-encoder`](jpeg_encoder) for encoding
//!
//! The decoder and encoder both support metadata extraction and saving.
//!
use jpeg_encoder::{ColorType, EncodingError, JfifWrite};
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteIoError, ZByteReaderTrait, ZByteWriterTrait, ZWriter};
use zune_core::colorspace::ColorSpace;
use zune_core::log::warn;
use zune_core::options::EncoderOptions;
use zune_jpeg::errors::DecodeErrors;
pub use zune_jpeg::{ImageInfo, JpegDecoder};

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecodeInto, DecoderTrait, EncoderTrait};

struct TempVt<'a, T: ZByteWriterTrait> {
    inner: &'a mut ZWriter<T>
}
impl<'a, T: ZByteWriterTrait> JfifWrite for TempVt<'a, T> {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), EncodingError> {
        self.inner.write_all(buf).map_err(|r| match r {
            ZByteIoError::StdIoError(e) => EncodingError::IoError(e),
            r => EncodingError::Write(format!("{:?}", r))
        })
    }
}
impl<T: ZByteReaderTrait> DecoderTrait for zune_jpeg::JpegDecoder<T> {
    fn decode(&mut self) -> Result<Image, crate::errors::ImageErrors> {
        let metadata = self.read_headers()?.unwrap();

        let pixels = self
            .decode()
            .map_err(<DecodeErrors as Into<ImageErrors>>::into)?;

        let colorspace = self.output_colorspace().unwrap();
        let (width, height) = self.dimensions().unwrap();

        let mut image = Image::from_u8(&pixels, width, height, colorspace);
        image.metadata = metadata;
        image.metadata.colorspace = self.output_colorspace().unwrap();
        Ok(image)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions().map(|dims| (dims.0, dims.1))
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.output_colorspace().unwrap()
    }

    fn name(&self) -> &'static str {
        "JPEG decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        self.decode_headers()
            .map_err(<DecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.dimensions().unwrap();

        let mut metadata = ImageMetadata {
            format: Some(ImageFormat::JPEG),
            colorspace: self.input_colorspace().unwrap(),
            depth: BitDepth::Eight,
            width: width,
            height: height,
            ..Default::default()
        };
        #[cfg(feature = "metadata")]
        {
            // see if we have an exif chunk
            if let Some(exif) = self.exif() {
                metadata.parse_raw_exif(exif)
            }
        }
        if let Some(icc) = self.icc_profile() {
            metadata.set_icc_chunk(icc);
        }

        Ok(Some(metadata))
    }
}

impl From<zune_jpeg::errors::DecodeErrors> for ImageErrors {
    fn from(from: zune_jpeg::errors::DecodeErrors) -> Self {
        let err = format!("jpg: {from:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}
// Okay I just need to really appreciate jpeg-encoder crate
// it's well written and covers quite a lot of things really
// well, so thank you Volker Ströbel

/// A simple JPEG encoder
#[derive(Copy, Clone, Default)]
pub struct JpegEncoder {
    options: Option<EncoderOptions>
}

impl JpegEncoder {
    /// Create a new encoder with default options
    pub fn new() -> JpegEncoder {
        JpegEncoder::default()
    }
    /// Create a new encoder with custom options
    pub fn new_with_options(options: EncoderOptions) -> JpegEncoder {
        JpegEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for JpegEncoder {
    fn name(&self) -> &'static str {
        "jpeg-encoder(vstroebel)"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T
    ) -> Result<usize, ImageErrors> {
        assert_eq!(
            image.depth(),
            BitDepth::Eight,
            "Unsupported bit depth{:?}",
            image.depth()
        );
        let pixels = &image.flatten_frames::<u8>()[0];

        if let Some(colorspace) = match_colorspace_to_colortype(image.colorspace()) {
            let max_dims = usize::from(u16::MAX);

            let (width, height) = image.dimensions();

            // check dimensions
            if (width > max_dims) || (height > max_dims) {
                let msg = format!(
                    "Too large image dimensions {} x {}, maximum is {} x {}",
                    width, height, max_dims, max_dims
                );
                return Err(ImgEncodeErrors::ImageEncodeErrors(msg).into());
            }
            let mut writer = ZWriter::new(sink);
            let temp_c = TempVt { inner: &mut writer };
            // create space for our encoder

            let options = create_options_for_encoder(self.options, image);

            // create encoder finally
            // vec<u8> supports write so we use that as our encoder
            let mut encoder = jpeg_encoder::Encoder::new(temp_c, options.quality());

            // add options
            encoder.set_progressive(options.jpeg_encode_progressive());
            encoder.set_optimized_huffman_tables(options.jpeg_optimized_huffman_tables());

            #[cfg(feature = "metadata")]
            {
                use exif::experimental::Writer;

                if options.strip_metadata() {
                    // explicit :)
                } else if let Some(metadata) = &image.metadata.exif {
                    let mut writer = Writer::new();
                    // write first tags for exif
                    let mut buf = std::io::Cursor::new(b"Exif\x00\x00".to_vec());
                    // set buffer position to be bytes written, to ensure we don't overwrite anything
                    buf.set_position(6);

                    for metadatum in metadata {
                        writer.push_field(metadatum);
                    }
                    let result = writer.write(&mut buf, false);
                    if result.is_ok() {
                        // add the exif tag to APP1 segment
                        encoder.add_app_segment(1, buf.get_ref())?;
                    } else {
                        warn!("Writing exif failed {:?}", result);
                    }
                }
            }

            encoder.encode(pixels, width as u16, height as u16, colorspace)?;

            Ok(writer.bytes_written())
        } else {
            Err(ImgEncodeErrors::UnsupportedColorspace(
                image.colorspace(),
                self.supported_colorspaces()
            )
            .into())
        }
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
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

    fn format(&self) -> ImageFormat {
        ImageFormat::JPEG
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth] {
        &[BitDepth::Eight]
    }

    fn default_depth(&self, _: BitDepth) -> BitDepth {
        BitDepth::Eight
    }

    fn set_options(&mut self, options: EncoderOptions) {
        self.options = Some(options)
    }
}

/// Match the library colorspace to jpeg color type
const fn match_colorspace_to_colortype(colorspace: ColorSpace) -> Option<ColorType> {
    match colorspace {
        ColorSpace::RGBA => Some(ColorType::Rgba),
        ColorSpace::RGB => Some(ColorType::Rgb),
        ColorSpace::YCbCr => Some(ColorType::Ycbcr),
        ColorSpace::Luma => Some(ColorType::Luma),
        ColorSpace::YCCK => Some(ColorType::Ycck),
        ColorSpace::CMYK => Some(ColorType::Cmyk),
        _ => None
    }
}

impl From<EncodingError> for ImageErrors {
    fn from(value: EncodingError) -> Self {
        ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(value.to_string()))
    }
}

impl<T> DecodeInto for JpegDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode_into(&mut self, buffer: &mut [u8]) -> Result<(), ImageErrors> {
        self.decode_into(buffer)
            .map_err(<DecodeErrors as Into<ImageErrors>>::into)?;

        Ok(())
    }

    fn output_buffer_size(&mut self) -> Result<usize, ImageErrors> {
        self.decode_headers()
            .map_err(<DecodeErrors as Into<ImageErrors>>::into)?;

        // unwrap is okay because we successfully decoded image headers
        Ok(self.output_buffer_size().unwrap())
    }
}
