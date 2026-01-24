#![cfg(feature = "webp")]

use std::io::{BufRead, Seek, Write};

use image_webp::{ColorType, DecodingError, WebPEncoder};
use jxl_oxide::color::WhitePoint::E;
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteWriterTrait;
use zune_core::colorspace::ColorSpace;
use zune_core::log::{trace, warn};

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::{AlphaState, ImageMetadata};
use crate::traits::{DecoderTrait, EncoderTrait};

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
            color_trc: None,
            default_gamma: None,
            width: w,
            height: h,
            colorspace: self.out_colorspace(),
            depth: BitDepth::Eight,
            format: Some(ImageFormat::WEBP),
            alpha: AlphaState::NonPreMultiplied,
            #[cfg(feature = "metadata")]
            exif: None,
            icc_chunk: None,
            is_linear: false
        }));
    }
}

pub struct ZuneWebpImageEncoder {}
impl ZuneWebpImageEncoder {
    pub fn new() -> Self {
        Self {}
    }
}
impl EncoderTrait for ZuneWebpImageEncoder {
    fn name(&self) -> &'static str {
        "Webp Image Encoder"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, mut sink: T
    ) -> Result<usize, ImageErrors> {
        trace!("Starting webp encoder");
        // First make options
        let options = create_options_for_encoder(None, image);
        let data = &image.to_u8()[0];
        let mut output = Vec::with_capacity(data.len());

        let mut encoder = WebPEncoder::new(&mut output);

        #[cfg(feature = "metadata")]
        {
            use exif::experimental::Writer;

            let mut inner_buff = std::io::Cursor::new(Vec::with_capacity(300));
            if !options.strip_metadata() {
                trace!("Writing exif data");
                if let Some(fields) = &image.metadata.exif {
                    let mut writer = Writer::new();

                    for metadatum in fields {
                        writer.push_field(metadatum);
                    }
                    let result = writer.write(&mut inner_buff, false);
                    if result.is_ok() {
                        encoder.set_exif_metadata(inner_buff.into_inner())
                    } else {
                        warn!("Writing exif failed {:?}", result);
                    }
                }
            }
        }

        let color_type = match options.colorspace() {
            ColorSpace::RGBA => ColorType::Rgba8,
            ColorSpace::RGB => ColorType::Rgb8,
            ColorSpace::Luma => ColorType::L8,
            ColorSpace::LumaA => ColorType::La8,
            _ => {
                return Err(ImageErrors::EncodeErrors(
                    ImgEncodeErrors::UnsupportedColorspace(
                        options.colorspace(),
                        self.supported_colorspaces()
                    )
                ))
            }
        };
        encoder
            .encode(
                data,
                options.width() as u32,
                options.height() as u32,
                color_type
            )
            .map_err(|e| ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(e.to_string())))?;

        sink.write_all_bytes(output.as_ref())?;
        Ok(output.len())
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::RGB,
            ColorSpace::RGBA,
            ColorSpace::Luma,
            ColorSpace::LumaA
        ]
    }
    fn format(&self) -> ImageFormat {
        ImageFormat::WEBP
    }
    fn supported_bit_depth(&self) -> &'static [BitDepth] {
        &[BitDepth::Eight]
    }
    fn default_depth(&self, depth: BitDepth) -> BitDepth {
        BitDepth::Eight
    }
}
