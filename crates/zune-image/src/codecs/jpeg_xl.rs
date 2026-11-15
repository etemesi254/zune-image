/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! JPEG-XL decoding and encoding support
//! This uses the delegate library [`zune-jpeg-xl`](zune_jpegxl)
//! for encoding and  [`jxl-oxide`](jxl_oxide) for decoding images
#![cfg_attr(feature = "docs", doc(cfg(feature = "jpeg-xl")))]
#![cfg(feature = "jpeg-xl")]
//! A simple jxl lossless encoder
//!
//! The encoder supports simple lossless image
//! (modular, no var-dct) with support for 8 bit and
//! 16 bit images with no palette support
//!
use std::io::Read;
use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;

pub use jxl_oxide;
use jxl_oxide::{JxlImage, PixelFormat, RenderResult};
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::bytestream::ZByteWriterTrait;
use zune_core::colorspace::{ColorPrimaries, ColorSpace, SingleColorPrimary};
use zune_core::log::{trace, warn};
use zune_core::options::{DecoderOptions, EncoderOptions};
pub use zune_jpegxl::*;

use crate::channel::Channel;
use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::frame::Frame;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecoderTrait, EncoderTrait};

/// A simple JXL encoder that ties the bridge between
/// Image struct and the [zune_jpegxl::SimpleJxlEncoder](zune_jpegxl::JxlSimpleEncoder)
#[derive(Default, Copy, Clone)]
pub struct JxlEncoder {
    options: Option<EncoderOptions>
}

impl JxlEncoder {
    /// Create a new encoder with default options
    ///
    /// Default options include 4 threads for encoding,and an effort
    /// od 4
    pub fn new() -> JxlEncoder {
        JxlEncoder::default()
    }
    /// Create new encoder with custom options
    pub fn new_with_options(options: EncoderOptions) -> JxlEncoder {
        JxlEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for JxlEncoder {
    fn name(&self) -> &'static str {
        "jxl-encoder"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T
    ) -> Result<usize, ImageErrors> {
        let options = create_options_for_encoder(self.options, image);

        let data = &image.to_u8()[0];

        let mut encoder = JxlSimpleEncoder::new(data, options);
        describe_colors(&mut encoder, image.metadata());

        let data = encoder
            .encode(sink)
            .map_err(<JxlEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::Luma,
            ColorSpace::LumaA,
            ColorSpace::RGBA,
            ColorSpace::RGB
        ]
    }

    fn format(&self) -> ImageFormat {
        ImageFormat::JPEG_XL
    }

    fn supported_bit_depth(&self) -> &'static [BitDepth] {
        &[BitDepth::Eight, BitDepth::Sixteen]
    }

    fn default_depth(&self, depth: BitDepth) -> BitDepth {
        match depth {
            BitDepth::Sixteen | BitDepth::Float32 => BitDepth::Sixteen,
            _ => BitDepth::Eight
        }
    }

    fn set_options(&mut self, options: EncoderOptions) {
        self.options = Some(options)
    }
}

/// The CIE x and y of a primary or white point.
const fn xy(x: f64, y: f64) -> SingleColorPrimary {
    SingleColorPrimary { x, y, z: 0.0 }
}

/// The primaries and white point a cICP `color_primaries` code names,
/// for those JPEG XL also names: 1 (BT.709 and sRGB), 9 (BT.2020 and
/// BT.2100), 11 (DCI-P3) and 12 (Display P3).
fn primaries_of_cicp(code: u8) -> Option<ColorPrimaries> {
    let d65 = xy(0.3127, 0.3290);
    let p3 = [xy(0.680, 0.320), xy(0.265, 0.690), xy(0.150, 0.060)];
    let ([red, green, blue], white_point) = match code {
        1 => ([xy(0.640, 0.330), xy(0.300, 0.600), xy(0.150, 0.060)], d65),
        9 => ([xy(0.708, 0.292), xy(0.170, 0.797), xy(0.131, 0.046)], d65),
        11 => (p3, xy(0.314, 0.351)),
        12 => (p3, d65),
        _ => return None
    };
    Some(ColorPrimaries {
        red,
        green,
        blue,
        white_point
    })
}

/// Has `encoder` name the transfer function, primaries and white point
/// `metadata` gives, as a PNG's cICP and cHRM chunks do, so that they
/// survive the conversion. cICP's primaries are taken before cHRM's,
/// as the PNG specification orders them. A transfer function JPEG XL
/// has no name for is left as sRGB's, with a warning, rather than
/// failing the encode; unknown cICP primaries without cHRM are left as
/// sRGB's too.
fn describe_colors(encoder: &mut JxlSimpleEncoder, metadata: &ImageMetadata) {
    if let Some(trc) = metadata.color_trc {
        if JxlSimpleEncoder::names_color_characteristics(trc) {
            encoder.set_color_characteristics(trc);
        } else {
            warn!("JPEG XL names no transfer function {:?}: writing sRGB's", trc);
        }
    }
    let primaries = (metadata.color_standard)
        .and_then(primaries_of_cicp)
        .or(metadata.color_primaries);
    if let Some(primaries) = primaries {
        encoder.set_color_primaries(primaries);
    }
}

impl From<JxlEncodeErrors> for ImgEncodeErrors {
    fn from(value: JxlEncodeErrors) -> Self {
        ImgEncodeErrors::ImageEncodeErrors(format!("{:?}", value))
    }
}

pub struct JxlDecoder {
    inner:   jxl_oxide::JxlImage,
    options: DecoderOptions
}

impl JxlDecoder {
    pub fn try_new<R: Read>(source: R, options: DecoderOptions) -> Result<JxlDecoder, ImageErrors> {
        let parser = jxl_oxide::JxlImage::builder()
            .read(source)
            .map_err(|x| ImageErrors::ImageDecodeErrors(format!("{:?}", x)))?;

        let decoder = JxlDecoder {
            inner: parser,
            options
        };
        Ok(decoder)
    }
}

impl DecoderTrait for JxlDecoder {
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        // by now headers have been decoded, so we can fetch these
        let metadata = self.read_headers()?;
        let (w, h) = <JxlDecoder as DecoderTrait>::dimensions(self).unwrap();
        let color = <JxlDecoder as DecoderTrait>::out_colorspace(self);

        let mut total_frames = vec![];

        if color == ColorSpace::Unknown {
            return Err(ImageErrors::ImageDecodeErrors(format!(
                "Encountered unknown/unsupported colorspace {:?}",
                self.inner.pixel_format()
            )));
        }
        trace!("Image colorspace: {:?}", color);
        trace!("Image dimensions: ({},{})", w, h);
        // check dimensions if bigger than supported
        if w > self.options.max_width() {
            let msg = format!(
                "Image width {}, greater than max set width {}",
                w,
                self.options.max_width()
            );
            return Err(ImageErrors::ImageDecodeErrors(msg));
        }
        if h > self.options.max_height() {
            let msg = format!(
                "Image height {}, greater than max set height {}",
                h,
                self.options.max_height()
            );
            return Err(ImageErrors::ImageDecodeErrors(msg));
        }

        let taken = if self.options.jxl_decode_animated() {
            self.inner.num_loaded_frames()
        } else {
            1
        };

        for frame in 0..taken {
            let render = self
                .inner
                .render_frame(frame)
                .map_err(|x| ImageErrors::ImageDecodeErrors(format!("{}", x)))?;

            // get the images
            let duration = render.duration();

            let im_plannar = render.image_planar();
            let mut frame_v = vec![];

            for channel in im_plannar {
                let mut chan = Channel::new_with_bit_type(
                    channel.width() * channel.height() * size_of::<f32>(),
                    BitType::F32
                );
                // copy the channel as plannar
                let c = chan.reinterpret_as_mut()?;
                c.copy_from_slice(channel.buf());
                // then store it in frame_v
                frame_v.push(chan);
            }
            let frame = Frame::new(frame_v);
            total_frames.push(frame);
        }
        // then create a new image
        let mut image = Image::new_frames(total_frames, BitDepth::Float32, w, h, color);
        if let Some(im_metadata) = metadata {
            image.metadata = im_metadata;
        }
        Ok(image)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        let (w, h) = (self.inner.width(), self.inner.height());
        Some((w as usize, h as usize))
    }

    fn out_colorspace(&self) -> ColorSpace {
        let format = self.inner.pixel_format();
        match format {
            PixelFormat::Gray => ColorSpace::Luma,
            PixelFormat::Graya => ColorSpace::LumaA,
            PixelFormat::Rgb => ColorSpace::RGB,
            PixelFormat::Rgba => ColorSpace::RGBA,
            PixelFormat::Cmyk => ColorSpace::CMYK,
            PixelFormat::Cmyka => ColorSpace::Unknown
        }
    }

    fn name(&self) -> &'static str {
        "jxl-decoder (tirr-c)"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, ImageErrors> {
        let (w, h) = <JxlDecoder as DecoderTrait>::dimensions(self).unwrap();
        let color = <JxlDecoder as DecoderTrait>::out_colorspace(self);

        let (width, height) = self.dimensions().unwrap();

        let icc = self.inner.rendered_icc();
        let metadata = ImageMetadata {
            format: Some(ImageFormat::JPEG_XL),
            colorspace: color,
            depth: BitDepth::Float32,
            width: width,
            height: height,
            icc_chunk: Some(icc),
            ..Default::default()
        };

        Ok(Some(metadata))
    }
}

#[cfg(test)]
mod tests {
    use jxl_oxide::color::{ColourEncoding, Primaries, TransferFunction, WhitePoint};
    use zune_core::colorspace::{ColorCharacteristics, ColorSpace};

    use super::*;
    use crate::traits::EncoderTrait;

    /// A 4 by 4 RGB ramp with `metadata`'s colors, encoded and its
    /// header's color encoding read back by jxl-oxide.
    fn round_trip(describe: impl FnOnce(&mut ImageMetadata)) -> ColourEncoding {
        let pixels: Vec<u8> = (0..48).map(|i| (i * 5) as u8).collect();
        let mut image = Image::from_u8(&pixels, 4, 4, ColorSpace::RGB);
        describe(image.metadata_mut());
        let mut bytes = Vec::new();
        JxlEncoder::new().encode(&image, &mut bytes).unwrap();
        let decoded = jxl_oxide::JxlImage::builder().read(&bytes[..]).unwrap();
        decoded.image_header().metadata.colour_encoding.clone()
    }

    /// `encoding`'s white point, primaries and transfer function.
    fn named(encoding: ColourEncoding) -> (WhitePoint, Primaries, TransferFunction) {
        match encoding {
            ColourEncoding::Enum(e) => (e.white_point, e.primaries, e.tf),
            other => panic!("expected an enum encoding, got {other:?}")
        }
    }

    #[test]
    fn cicp_transfer_and_primaries_reach_the_header() {
        let got = named(round_trip(|m| {
            m.color_trc = Some(ColorCharacteristics::PQ);
            m.color_standard = Some(9);
        }));
        assert_eq!(got.0, WhitePoint::D65);
        assert_eq!(got.1, Primaries::Bt2100);
        assert_eq!(got.2, TransferFunction::Pq);
    }

    #[test]
    fn cicp_dci_p3_keeps_its_white_point() {
        let got = named(round_trip(|m| {
            m.color_trc = Some(ColorCharacteristics::Smpte428);
            m.color_standard = Some(11);
        }));
        assert_eq!(got.0, WhitePoint::Dci);
        assert_eq!(got.1, Primaries::P3);
        assert_eq!(got.2, TransferFunction::Dci);
    }

    #[test]
    fn chrm_primaries_reach_the_header_without_cicp() {
        let p3 = primaries_of_cicp(12).unwrap();
        let got = named(round_trip(|m| m.color_primaries = Some(p3)));
        assert_eq!(got.0, WhitePoint::D65);
        assert_eq!(got.1, Primaries::P3);
        assert_eq!(got.2, TransferFunction::Srgb);
    }

    #[test]
    fn cicp_primaries_come_before_chrm() {
        let p3 = primaries_of_cicp(12).unwrap();
        let got = named(round_trip(|m| {
            m.color_primaries = Some(p3);
            m.color_standard = Some(1);
        }));
        assert_eq!(got.1, Primaries::Srgb);
    }

    #[test]
    fn an_unnamed_transfer_function_is_written_as_srgb() {
        let got = named(round_trip(|m| {
            m.color_trc = Some(ColorCharacteristics::Log100);
        }));
        assert_eq!(got.1, Primaries::Srgb);
        assert_eq!(got.2, TransferFunction::Srgb);
    }

    /// The CRC-32 a PNG chunk ends with, of its type and data.
    #[cfg(feature = "png")]
    fn crc(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
            }
        }
        !crc
    }

    /// A PNG chunk of `kind` holding `data`.
    #[cfg(feature = "png")]
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc(&body).to_be_bytes());
        out
    }

    #[test]
    #[cfg(feature = "png")]
    fn a_png_s_cicp_and_chrm_survive_conversion_to_jpeg_xl() {
        use zune_core::bytestream::ZCursor;
        use zune_core::options::DecoderOptions;

        let pixels: Vec<u8> = (0..48).map(|i| (i * 5) as u8).collect();
        let image = Image::from_u8(&pixels, 4, 4, ColorSpace::RGB);
        let mut png = Vec::new();
        crate::codecs::png::PngEncoder::new()
            .encode(&image, &mut png)
            .unwrap();
        // After the signature and IHDR (8 + 25 bytes): cICP naming
        // BT.2020's primaries with HLG, and cHRM giving P3's, which
        // cICP's come before.
        let p3 = primaries_of_cicp(12).unwrap();
        let chrm: Vec<u8> = [p3.white_point, p3.red, p3.green, p3.blue]
            .iter()
            .flat_map(|c| [c.x, c.y])
            .flat_map(|v| ((v * 100_000.0).round() as u32).to_be_bytes())
            .collect();
        let mut tagged = png[..33].to_vec();
        tagged.extend(chunk(b"cICP", &[9, 18, 0, 1]));
        tagged.extend(chunk(b"cHRM", &chrm));
        tagged.extend_from_slice(&png[33..]);

        let read = Image::read(ZCursor::new(&tagged), DecoderOptions::default()).unwrap();
        let mut bytes = Vec::new();
        JxlEncoder::new().encode(&read, &mut bytes).unwrap();
        let decoded = jxl_oxide::JxlImage::builder().read(&bytes[..]).unwrap();
        let got = named(decoded.image_header().metadata.colour_encoding.clone());
        assert_eq!(got, (WhitePoint::D65, Primaries::Bt2100, TransferFunction::Hlg));
    }

    #[test]
    fn no_color_metadata_is_srgb() {
        let got = named(round_trip(|_| ()));
        assert_eq!(got, (WhitePoint::D65, Primaries::Srgb, TransferFunction::Srgb));
    }
}
