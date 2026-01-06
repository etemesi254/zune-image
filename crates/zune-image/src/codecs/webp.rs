#![cfg(feature = "webp")]

use std::io::{BufRead, Seek};

use image_webp::DecodingError;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;

use crate::codecs::ImageFormat;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::metadata::{AlphaState, ImageMetadata};
use crate::traits::DecoderTrait;

pub struct ZuneWebpDecoder<T: BufRead + Seek> {
    inner: image_webp::WebPDecoder<T>
}
impl<T: BufRead + Seek> ZuneWebpDecoder<T> {
    pub fn new(r: T) -> Result<Self, ImageErrors> {
        let decoder = image_webp::WebPDecoder::new(r).map_err(|e| {
            println!("{:?}", e);
            ImageErrors::ImageDecodeErrors(e.to_string())
        })?;
        return Ok(ZuneWebpDecoder { inner: decoder });
    }
}
impl<T: BufRead + Seek> DecoderTrait for ZuneWebpDecoder<T> {
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let (w, h) = self.dimensions().expect("Failed to determine dimensions");
        let mut raw_pixels = vec![
            0_u8;
            self.inner
                .output_buffer_size()
                .expect("Max output size greater than usize max")
        ];
        self.inner
            .read_image(&mut raw_pixels)
            .map_err(|e| ImageErrors::ImageDecodeErrors(e.to_string()))?;

        let mut img = Image::from_u8(&raw_pixels, w, h, self.out_colorspace());
        img.metadata.format = Some(ImageFormat::WEBP);

        Ok(img)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        let (w, h) = self.inner.dimensions();

        Some((w as usize, h as usize))
    }

    fn out_colorspace(&self) -> ColorSpace {
        // According to the docs, if image has alpha
        // colorspace is rgba, if not RGB
        match self.inner.has_alpha() {
            true => ColorSpace::RGBA,
            false => ColorSpace::RGB
        }
    }

    fn name(&self) -> &'static str {
        "Webp Decoder (image-rs)"
    }
    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, ImageErrors> {
        let (w, h) = self.dimensions().expect("Failed to determine dimensions");
        return Ok(Some(ImageMetadata {
            color_trc:     None,
            default_gamma: None,
            width:         w,
            height:        h,
            colorspace:    self.out_colorspace(),
            depth:         BitDepth::Eight,
            format:        Some(ImageFormat::WEBP),
            alpha:         AlphaState::NonPreMultiplied,
            #[cfg(feature = "metadata")]
            exif:          None,
            icc_chunk:     None,
            is_linear:     false
        }));
    }
}
