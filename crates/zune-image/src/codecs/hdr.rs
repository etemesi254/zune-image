/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(feature = "hdr")]
//! Radiance HDR decoding support
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteReaderTrait, ZByteWriterTrait};
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
pub use zune_hdr::*;

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecoderTrait, EncoderTrait};

impl<T> DecoderTrait for HdrDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let bytes = self.decode()?;
        let (width, height) = self.dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        Ok(Image::from_f32(&bytes, width, height, colorspace))
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions()
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.get_colorspace().unwrap()
    }

    fn name(&self) -> &'static str {
        "HDR decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, ImageErrors> {
        self.decode_headers()?;

        let (width, height) = self.dimensions().unwrap();

        let metadata = ImageMetadata {
            width,
            height,
            colorspace: ColorSpace::RGB,
            depth: BitDepth::Float32,
            format: Some(ImageFormat::HDR),
            ..Default::default()
        };
        Ok(Some(metadata))
    }
}

impl From<HdrDecodeErrors> for ImageErrors {
    fn from(value: HdrDecodeErrors) -> Self {
        Self::ImageDecodeErrors(format!("hdr: {value:?}"))
    }
}

#[derive(Default)]
pub struct HdrEncoder {
    options: Option<EncoderOptions>
}

impl HdrEncoder {
    pub fn new() -> HdrEncoder {
        Self::default()
    }
    pub fn new_with_options(options: EncoderOptions) -> HdrEncoder {
        HdrEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for HdrEncoder {
    fn name(&self) -> &'static str {
        "Hdr"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T
    ) -> Result<usize, ImageErrors> {
        let options = create_options_for_encoder(self.options, image);

        assert_eq!(image.depth(), BitDepth::Float32);

        let data = &image.flatten_frames()[0];

        let encoder_options = zune_hdr::HdrEncoder::new(data, options);

        let data = encoder_options
            .encode(sink)
            .map_err(<HdrEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[ColorSpace::RGB]
    }

    fn format(&self) -> ImageFormat {
        ImageFormat::HDR
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth] {
        &[BitDepth::Float32]
    }

    fn default_depth(&self, _: BitDepth) -> BitDepth {
        BitDepth::Float32
    }

    fn default_colorspace(&self, _: ColorSpace) -> ColorSpace {
        ColorSpace::RGB
    }

    fn set_options(&mut self, opts: EncoderOptions) {
        self.options = Some(opts)
    }
}

impl From<HdrEncodeErrors> for ImgEncodeErrors {
    fn from(value: HdrEncodeErrors) -> Self {
        ImgEncodeErrors::ImageEncodeErrors(format!("HDR: {:?}", value))
    }
}
