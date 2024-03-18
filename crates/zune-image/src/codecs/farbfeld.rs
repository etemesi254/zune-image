/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(feature = "farbfeld")]
//! Farbfeld decoding and encoding support
//!
//! This uses the delegate library [`zune-farbfeld`](zune_farbfeld)
//! for encoding and decoding images
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteReaderTrait, ZByteWriterTrait};
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
pub use zune_farbfeld::*;

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecodeInto, DecoderTrait, EncoderTrait};

impl<T> DecoderTrait for FarbFeldDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let pixels = self
            .decode()
            .map_err(|e| ImageErrors::ImageDecodeErrors(format!("{:?}", e)))?;
        let colorspace = self.colorspace();
        let (width, height) = self.dimensions().unwrap();

        let mut image = Image::from_u16(&pixels, width, height, colorspace);

        image.metadata.format = Some(ImageFormat::Farbfeld);

        Ok(image)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions()
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace()
    }

    fn name(&self) -> &'static str {
        "Farbfeld Decoder"
    }

    fn is_experimental(&self) -> bool {
        true
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        self.decode_headers()
            .map_err(|e| ImageErrors::ImageDecodeErrors(format!("{:?}", e)))?;

        let (width, height) = self.dimensions().unwrap();
        let depth = self.bit_depth();

        let metadata = ImageMetadata {
            format: Some(ImageFormat::Farbfeld),
            colorspace: self.colorspace(),
            depth: depth,
            width: width,
            height: height,
            ..Default::default()
        };

        Ok(Some(metadata))
    }
}

/// A small wrapper against the Farbfeld encoder that ties
/// the bridge between Image struct and the buffer
/// which [zune_farbfeld::FarbFeldEncoder](zune_farbfeld::FarbFeldEncoder)
/// understands
#[derive(Default)]
pub struct FarbFeldEncoder {
    options: Option<EncoderOptions>
}

impl FarbFeldEncoder {
    /// Create a new encoder
    pub fn new() -> FarbFeldEncoder {
        FarbFeldEncoder::default()
    }
    /// Create a new encoder with specified options
    pub fn new_with_options(options: EncoderOptions) -> FarbFeldEncoder {
        FarbFeldEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for FarbFeldEncoder {
    fn name(&self) -> &'static str {
        "farbfeld"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T
    ) -> Result<usize, ImageErrors> {
        let options = create_options_for_encoder(self.options, image);

        assert_eq!(image.depth(), BitDepth::Sixteen);

        let data = &image.to_u8()[0];

        let encoder = zune_farbfeld::FarbFeldEncoder::new(data, options);

        let data = encoder
            .encode(sink)
            .map_err(<FarbFeldEncoderErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[ColorSpace::RGBA]
    }

    fn format(&self) -> ImageFormat {
        ImageFormat::Farbfeld
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth] {
        &[BitDepth::Sixteen]
    }

    fn default_depth(&self, _: BitDepth) -> BitDepth {
        BitDepth::Sixteen
    }

    fn default_colorspace(&self, _: ColorSpace) -> ColorSpace {
        ColorSpace::RGBA
    }
    fn set_options(&mut self, opts: EncoderOptions) {
        self.options = Some(opts)
    }
}

impl From<FarbFeldEncoderErrors> for ImgEncodeErrors {
    fn from(value: FarbFeldEncoderErrors) -> Self {
        ImgEncodeErrors::ImageEncodeErrors(format!("{:?}", value))
    }
}
