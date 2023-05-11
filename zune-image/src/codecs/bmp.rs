/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
#![cfg(feature = "bmp")]

use zune_bmp::{BmpDecoder, BmpDecoderErrors};
use zune_core::bytestream::ZReaderTrait;
use zune_core::colorspace::ColorSpace;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::DecoderTrait;

impl<T> DecoderTrait for BmpDecoder<T>
where
    T: ZReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors>
    {
        let pixels = self.decode()?;
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        Ok(Image::from_u8(&pixels, width, height, colorspace))
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
        "BMP Decoder"
    }
}

impl From<BmpDecoderErrors> for ImageErrors
{
    fn from(value: BmpDecoderErrors) -> Self
    {
        Self::ImageDecodeErrors(format!("bmp: {:?}", value))
    }
}
