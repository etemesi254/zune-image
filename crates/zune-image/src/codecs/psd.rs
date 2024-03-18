/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Photoshop (.psd) decoding support
//!
//! This uses the delegate library [`zune-psd`](zune_psd)
//! for decoding images
#![cfg(feature = "psd")]

use zune_core::bytestream::ZByteReaderTrait;
use zune_core::colorspace::ColorSpace;
use zune_core::result::DecodingResult;
pub use zune_psd::*;

use crate::codecs::ImageFormat;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::DecoderTrait;

impl<T> DecoderTrait for PSDDecoder<T>
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
            _ => unreachable!()
        };
        // set metadata details
        image.metadata.format = Some(ImageFormat::PSD);

        Ok(image)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions()
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace().unwrap()
    }

    fn name(&self) -> &'static str {
        "PSD Decoder"
    }

    fn is_experimental(&self) -> bool {
        true
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        self.decode_headers()
            .map_err(<errors::PSDDecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.dimensions().unwrap();
        let depth = self.bit_depth().unwrap();

        let metadata = ImageMetadata {
            format: Some(ImageFormat::PSD),
            colorspace: self.colorspace().unwrap(),
            depth: depth,
            width: width,
            height: height,
            ..Default::default()
        };

        Ok(Some(metadata))
    }
}

impl From<zune_psd::errors::PSDDecodeErrors> for ImageErrors {
    fn from(error: zune_psd::errors::PSDDecodeErrors) -> Self {
        let err = format!("psd: {error:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}
