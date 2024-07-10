/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! BMP decoding support
//!
//! Decoding is done by the delegate library [zune-bmp](zune_bmp)
#![cfg(feature = "bmp")]

pub use zune_bmp::*;
use zune_core::bytestream::ZByteReaderTrait;
use zune_core::colorspace::ColorSpace;

use crate::codecs::ImageFormat;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecodeInto, DecoderTrait};

impl<T> DecoderTrait for BmpDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let pixels = self.decode()?;
        let (width, height) = self.dimensions().unwrap();
        let colorspace = self.colorspace().unwrap();

        Ok(Image::from_u8(&pixels, width, height, colorspace))
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions()
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace().unwrap()
    }

    fn name(&self) -> &'static str {
        "BMP Decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, ImageErrors> {
        self.decode_headers()?;

        let (width, height) = self.dimensions().unwrap();
        let depth = self.depth();

        let metadata = ImageMetadata {
            format: Some(ImageFormat::BMP),
            colorspace: self.colorspace().expect("Impossible"),
            depth: depth,
            width: width,
            height: height,
            icc_chunk: self.icc_profile().cloned(),
            ..Default::default()
        };

        Ok(Some(metadata))
    }
}

impl From<BmpDecoderErrors> for ImageErrors {
    fn from(value: BmpDecoderErrors) -> Self {
        Self::ImageDecodeErrors(format!("bmp: {:?}", value))
    }
}

impl<T> DecodeInto for BmpDecoder<T>
where
    T: ZByteReaderTrait
{
    type BufferType = u8;

    fn decode_into(&mut self, buffer: &mut [Self::BufferType]) -> Result<(), ImageErrors> {
        self.decode_into(buffer)
            .map_err(<BmpDecoderErrors as Into<ImageErrors>>::into)?;

        Ok(())
    }

    fn output_buffer_size(&mut self) -> Result<usize, ImageErrors> {
        self.decode_headers()
            .map_err(<BmpDecoderErrors as Into<ImageErrors>>::into)?;

        // unwrap is okay because we successfully decoded image headers
        Ok(self.output_buf_size().unwrap())
    }
}
