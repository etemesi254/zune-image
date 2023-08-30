/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(feature = "psd")]

use zune_core::bytestream::ZReaderTrait;
use zune_core::colorspace::ColorSpace;
use zune_core::result::DecodingResult;
pub use zune_psd::*;

use crate::codecs::ImageFormat;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::DecoderTrait;

impl<T> DecoderTrait<T> for PSDDecoder<T>
where
    T: ZReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let pixels = self.decode()?;

        let depth = self.get_bit_depth().unwrap();
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        let mut image = match pixels {
            DecodingResult::U8(data) => Image::from_u8(&data, width, height, colorspace),
            DecodingResult::U16(data) => Image::from_u16(&data, width, height, colorspace),
            _ => unreachable!()
        };
        // set metadata details
        image.metadata.format = Some(ImageFormat::PSD);

        Ok(image)
    }

    fn get_dimensions(&self) -> Option<(usize, usize)> {
        self.get_dimensions()
    }

    fn get_out_colorspace(&self) -> ColorSpace {
        self.get_colorspace().unwrap()
    }

    fn get_name(&self) -> &'static str {
        "PSD Decoder"
    }

    fn is_experimental(&self) -> bool {
        true
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        self.decode_headers()
            .map_err(<errors::PSDDecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_bit_depth().unwrap();

        let metadata = ImageMetadata {
            format: Some(ImageFormat::PSD),
            colorspace: self.get_colorspace().unwrap(),
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
