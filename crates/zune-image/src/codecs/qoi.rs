/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! QOI decoding and encoding support
//!
//! This uses the delegate library [`zune-qoi`](zune_qoi)
//! for encoding and decoding images
#![cfg(feature = "qoi")]

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteReaderTrait, ZByteWriterTrait};
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
pub use zune_qoi::*;

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecodeInto, DecoderTrait, EncoderTrait};

impl<T> DecoderTrait for QoiDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let pixels = self.decode()?;
        // safe because these are none when we haven't decoded.
        let colorspace = self.colorspace().unwrap();
        let (width, height) = self.dimensions().unwrap();

        let depth = self.bit_depth();

        let mut image = Image::from_u8(&pixels, width, height, colorspace);

        // set metadata details
        image.metadata.format = Some(ImageFormat::QOI);

        Ok(image)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions()
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace().unwrap()
    }

    fn name(&self) -> &'static str {
        "QOI Decoder"
    }

    fn is_experimental(&self) -> bool {
        true
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        self.decode_headers()
            .map_err(<QoiErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.dimensions().unwrap();
        let depth = self.bit_depth();

        let metadata = ImageMetadata {
            format: Some(ImageFormat::QOI),
            colorspace: self.colorspace().unwrap(),
            depth: depth,
            width: width,
            height: height,
            ..Default::default()
        };

        Ok(Some(metadata))
    }
}

#[derive(Copy, Clone, Default)]
pub struct QoiEncoder {
    options: Option<EncoderOptions>
}

impl QoiEncoder {
    pub fn new() -> QoiEncoder {
        QoiEncoder::default()
    }

    pub fn new_with_options(options: EncoderOptions) -> QoiEncoder {
        QoiEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for QoiEncoder {
    fn name(&self) -> &'static str {
        "QOI Encoder"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T
    ) -> Result<usize, ImageErrors> {
        let options = create_options_for_encoder(self.options, image);

        let data = &image.to_u8()[0];

        let mut qoi_encoder = zune_qoi::QoiEncoder::new(data, options);

        let bytes_written = qoi_encoder
            .encode(sink)
            .map_err(<QoiEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(bytes_written)
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[ColorSpace::RGBA, ColorSpace::RGB]
    }

    fn format(&self) -> ImageFormat {
        ImageFormat::QOI
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth] {
        &[BitDepth::Eight]
    }

    fn default_depth(&self, _: BitDepth) -> BitDepth {
        BitDepth::Eight
    }

    fn default_colorspace(&self, colorspace: ColorSpace) -> ColorSpace {
        // if colorspace has an alpha channel,
        // we want to preserve it in the final encoder
        if colorspace.has_alpha() {
            ColorSpace::RGBA
        } else {
            // otherwise, just stick up to the one we know
            ColorSpace::RGB
        }
    }
    fn set_options(&mut self, opts: EncoderOptions) {
        self.options = Some(opts)
    }
}

impl From<zune_qoi::QoiErrors> for ImageErrors {
    fn from(error: zune_qoi::QoiErrors) -> Self {
        let err = format!("qoi: {error:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}

impl From<zune_qoi::QoiEncodeErrors> for ImgEncodeErrors {
    fn from(error: zune_qoi::QoiEncodeErrors) -> Self {
        let err = format!("qoi: {error:?}");

        ImgEncodeErrors::Generic(err)
    }
}

impl<T> DecodeInto for QoiDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode_into(&mut self, buffer: &mut [u8]) -> Result<(), ImageErrors> {
        self.decode_into(buffer)?;

        Ok(())
    }

    fn output_buffer_size(&mut self) -> Result<usize, ImageErrors> {
        self.decode_headers()?;

        // unwrap is okay because we successfully decoded image headers
        Ok(self.output_buffer_size().unwrap())
    }
}
