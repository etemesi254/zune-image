/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(feature = "ppm")]
//! Represents a PPM and PAL image encoder
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteReaderTrait, ZByteWriterTrait};
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_core::result::DecodingResult;
pub use zune_ppm::{PPMDecodeErrors, PPMDecoder, PPMEncodeErrors, PPMEncoder as PPMEnc};

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecoderTrait, EncoderTrait};

#[derive(Copy, Clone, Default)]
pub struct PPMEncoder {
    options: Option<EncoderOptions>
}

impl PPMEncoder {
    pub fn new() -> PPMEncoder {
        PPMEncoder { options: None }
    }
    pub fn new_with_options(options: EncoderOptions) -> PPMEncoder {
        PPMEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for PPMEncoder {
    fn name(&self) -> &'static str {
        "PPM Encoder"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T
    ) -> Result<usize, ImageErrors> {
        let options = create_options_for_encoder(self.options, image);

        let data = &image.to_u8()[0];

        let ppm_encoder = PPMEnc::new(data, options);

        let bytes_written = ppm_encoder
            .encode(sink)
            .map_err(<PPMEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(bytes_written)
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::RGB,  // p7
            ColorSpace::Luma, // p7
            ColorSpace::RGBA, // p7
            ColorSpace::LumaA
        ]
    }

    fn format(&self) -> ImageFormat {
        ImageFormat::PPM
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth] {
        &[BitDepth::Sixteen, BitDepth::Eight]
    }

    /// Get appropriate depth for this image
    ///
    /// Float32 types, they are converted to Float16 types
    fn default_depth(&self, depth: BitDepth) -> BitDepth {
        match depth {
            BitDepth::Float32 | BitDepth::Sixteen => BitDepth::Sixteen,
            _ => BitDepth::Eight
        }
    }
    fn set_options(&mut self, opts: EncoderOptions) {
        self.options = Some(opts)
    }
}

impl<T> DecoderTrait for PPMDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let pixels = self.decode()?;

        let depth = self.bit_depth().unwrap();
        let (width, height) = self.dimensions().unwrap();
        let colorspace = self.colorspace().unwrap();

        let mut image = match pixels {
            DecodingResult::U8(data) => Image::from_u8(&data, width, height, colorspace),
            DecodingResult::U16(data) => Image::from_u16(&data, width, height, colorspace),
            DecodingResult::F32(data) => Image::from_f32(&data, width, height, colorspace),
            _ => unreachable!()
        };

        // set metadata details
        image.metadata.format = Some(ImageFormat::PPM);

        Ok(image)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions()
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace().unwrap_or(ColorSpace::Unknown)
    }

    fn name(&self) -> &'static str {
        "PPM Decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        self.decode_headers()
            .map_err(<PPMDecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.dimensions().unwrap();
        let depth = self.bit_depth().unwrap();

        let metadata = ImageMetadata {
            format: Some(ImageFormat::PPM),
            colorspace: self.colorspace().unwrap(),
            depth: depth,
            width: width,
            height: height,
            ..Default::default()
        };

        Ok(Some(metadata))
    }
}

#[cfg(feature = "ppm")]
impl From<zune_ppm::PPMDecodeErrors> for ImageErrors {
    fn from(from: zune_ppm::PPMDecodeErrors) -> Self {
        let err = format!("ppm: {from:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}

#[cfg(feature = "ppm")]
impl From<zune_ppm::PPMEncodeErrors> for ImgEncodeErrors {
    fn from(error: zune_ppm::PPMEncodeErrors) -> Self {
        let err = format!("ppm: {error:?}");

        ImgEncodeErrors::ImageEncodeErrors(err)
    }
}
