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
use jxl_oxide::{PixelFormat, RenderResult};
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::bytestream::ZByteWriterTrait;
use zune_core::colorspace::ColorSpace;
use zune_core::log::trace;
use zune_core::options::{DecoderOptions, EncoderOptions};
pub use zune_jpegxl::*;

use crate::channel::Channel;
use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::frame::Frame;
use crate::image::Image;
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

        let encoder = JxlSimpleEncoder::new(data, options);

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

impl From<JxlEncodeErrors> for ImgEncodeErrors {
    fn from(value: JxlEncodeErrors) -> Self {
        ImgEncodeErrors::ImageEncodeErrors(format!("{:?}", value))
    }
}

pub struct JxlDecoder<R: Read> {
    inner:   jxl_oxide::JxlImage<R>,
    options: DecoderOptions
}

impl<R: Read> JxlDecoder<R> {
    pub fn try_new(source: R, options: DecoderOptions) -> Result<JxlDecoder<R>, ImageErrors> {
        let parser = jxl_oxide::JxlImage::from_reader(source)
            .map_err(|x| ImageErrors::ImageDecodeErrors(format!("{:?}", x)))?;

        let decoder = JxlDecoder {
            inner: parser,
            options
        };
        Ok(decoder)
    }
}

impl<R> DecoderTrait for JxlDecoder<R>
where
    R: Read
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        // by now headers have been decoded, so we can fetch these
        let (w, h) = <JxlDecoder<R> as DecoderTrait>::dimensions(self).unwrap();
        let color = <JxlDecoder<R> as DecoderTrait>::out_colorspace(self);

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

        loop {
            let result = self
                .inner
                .render_next_frame()
                .map_err(|x| ImageErrors::ImageDecodeErrors(format!("{}", x)))?;

            match result {
                RenderResult::Done(render) => {
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
                RenderResult::NeedMoreData => {
                    sleep(Duration::new(1, 0));
                    // return to the loop
                }
                RenderResult::NoMoreFrames => break
            }
            if !self.options.jxl_decode_animated() {
                // we won't be decoding animated, so don't decode the next frame
                break;
            }
        }
        // then create a new image
        Ok(Image::new_frames(
            total_frames,
            BitDepth::Float32,
            w,
            h,
            color
        ))
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
}
