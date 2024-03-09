/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(feature = "png")]
#![allow(unused_variables)]

//! Represents an png image decoder
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReaderTrait;
use zune_core::colorspace::ColorSpace;
use zune_core::log::warn;
use zune_core::options::EncoderOptions;
use zune_core::result::DecodingResult;
pub use zune_png::*;

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::ImageErrors;
use crate::errors::ImageErrors::ImageDecodeErrors;
use crate::frame::Frame;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecoderTrait, EncoderTrait};

impl<T> DecoderTrait for PngDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let metadata = self.read_headers()?.unwrap();

        let depth = self.get_depth().unwrap();
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        if self.is_animated() && self.get_options().png_decode_animated() {
            // decode apng frames
            //let mut previous_frame
            let info = self.get_info().unwrap().clone();
            // the output, since we know that no frame will be bigger than the width and height, we can
            // set this up outside of the loop.
            let mut output = vec![0; info.width * info.height * colorspace.num_components()];
            let mut output_frames = Vec::new();
            while self.more_frames() {
                self.decode_headers().unwrap();

                let mut frame = self.frame_info().unwrap();
                if frame.dispose_op == DisposeOp::Previous {
                    // we don't clear our buffer, so output always contains the previous frame
                    //
                    // this means that there is no need to store the previous frame and copy it
                    frame.dispose_op = DisposeOp::None;
                }
                let pix = self.decode().unwrap();
                match pix {
                    DecodingResult::U8(pix) => {
                        post_process_image(
                            &info,
                            colorspace,
                            &frame,
                            &pix,
                            None,
                            &mut output,
                            None,
                        )?;
                        let duration = f64::from(frame.delay_num) / f64::from(frame.delay_denom);
                        // then build a frame from that
                        let im_frame = Frame::from_u8(&output, colorspace, usize::from(frame.delay_num),usize::from(frame.delay_denom));
                        output_frames.push(im_frame);
                    }
                    _ => return Err(ImageDecodeErrors("The current image is an  Animated PNG but has a depth of 16, such an image isn't supported".to_string()))
                }
            }
            let mut image = Image::new_frames(output_frames, depth, width, height, colorspace);
            image.metadata = metadata;

            Ok(image)
        } else {
            let pixels = self
                .decode()
                .map_err(<error::PngDecodeErrors as Into<ImageErrors>>::into)?;

            let mut image = match pixels {
                DecodingResult::U8(data) => Image::from_u8(&data, width, height, colorspace),
                DecodingResult::U16(data) => Image::from_u16(&data, width, height, colorspace),
                _ => unreachable!()
            };
            // metadata
            image.metadata = metadata;

            Ok(image)
        }
    }
    fn dimensions(&self) -> Option<(usize, usize)> {
        self.get_dimensions()
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.get_colorspace().unwrap()
    }

    fn name(&self) -> &'static str {
        "PNG Decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        self.decode_headers()
            .map_err(<error::PngDecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.get_dimensions().unwrap();
        let depth = self.get_depth().unwrap();

        let mut metadata = ImageMetadata {
            format: Some(ImageFormat::PNG),
            colorspace: self.get_colorspace().unwrap(),
            depth: depth,
            width: width,
            height: height,
            default_gamma: self.get_info().unwrap().gamma,
            ..Default::default()
        };
        #[cfg(feature = "metadata")]
        {
            let info = self.get_info().unwrap();
            // see if we have an exif chunk
            if let Some(exif) = &info.exif {
                metadata.parse_raw_exif(exif)
            }
        }

        Ok(Some(metadata))
    }
}

impl From<zune_png::error::PngDecodeErrors> for ImageErrors {
    fn from(from: zune_png::error::PngDecodeErrors) -> Self {
        let err = format!("png: {from:?}");

        ImageErrors::ImageDecodeErrors(err)
    }
}

#[derive(Default)]
pub struct PngEncoder {
    options: Option<EncoderOptions>
}

impl PngEncoder {
    pub fn new() -> PngEncoder {
        PngEncoder::default()
    }
    pub fn new_with_options(options: EncoderOptions) -> PngEncoder {
        PngEncoder {
            options: Some(options)
        }
    }
}

impl EncoderTrait for PngEncoder {
    fn name(&self) -> &'static str {
        "PNG encoder"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImageErrors> {
        let options = create_options_for_encoder(self.options, image);

        let frame = &image.to_u8_be()[0];

        let mut encoder = zune_png::PngEncoder::new(frame, options);

        let mut buf = std::io::Cursor::new(vec![]);

        #[cfg(feature = "metadata")]
        {
            use exif::experimental::Writer;

            if !options.strip_metadata() {
                if let Some(fields) = &image.metadata.exif {
                    let mut writer = Writer::new();

                    for metadatum in fields {
                        writer.push_field(metadatum);
                    }
                    let result = writer.write(&mut buf, false);
                    if result.is_ok() {
                        encoder.add_exif_segment(buf.get_ref());
                    } else {
                        warn!("Writing exif failed {:?}", result);
                    }
                }
            }
        }
        Ok(encoder.encode())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::Luma,
            ColorSpace::LumaA,
            ColorSpace::RGB,
            ColorSpace::RGBA
        ]
    }

    fn format(&self) -> ImageFormat {
        ImageFormat::PNG
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
    fn set_options(&mut self, opts: EncoderOptions) {
        self.options = Some(opts)
    }
}
