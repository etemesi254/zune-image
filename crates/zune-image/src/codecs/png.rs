/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
#![cfg_attr(feature = "docs", doc(cfg(feature = "png")))]
#![cfg(feature = "png")]
#![allow(unused_variables)]

//! Represents an png image decoder and encoder
use std::io::Cursor;

use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::bytestream::{ZByteReaderTrait, ZByteWriterTrait};
use zune_core::colorspace::{ColorCharacteristics, ColorPrimaries, ColorSpace, SingleColorPrimary};
use zune_core::log::warn;
use zune_core::options::EncoderOptions;
use zune_core::result::DecodingResult;
use zune_png::error::PngDecodeErrors;
use zune_png::*;

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::ImageErrors;
use crate::errors::ImageErrors::ImageDecodeErrors;
use crate::errors::ImgEncodeErrors::ImageEncodeErrors;
use crate::frame::{Endianness, Frame};
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::{DecodeInto, DecoderTrait, EncoderTrait};
pub use zune_png::PngDecoder;

impl<T> DecoderTrait for PngDecoder<T>
where
    T: ZByteReaderTrait,
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let metadata = self.read_headers()?.unwrap();

        let depth = self.depth().unwrap();
        let (width, height) = self.dimensions().unwrap();
        let colorspace = self.colorspace().unwrap();

        if self.is_animated() && self.options().png_decode_animated() {
            // decode apng frames
            //let mut previous_frame
            let info = self.info().unwrap().clone();

            // Allocate both the main canvas and the backup canvas
            let buffer_size = info.width * info.height * colorspace.num_components();
            let mut output_frames = Vec::new();

            match self.depth().unwrap().bit_type() {
                BitType::U8 => {
                    let mut output = vec![0; buffer_size];
                    let mut apng_ctx = ApngContext::<u8>::new(self.info().unwrap(), colorspace);

                    while self.more_frames() {
                        self.decode_headers()?;

                        // We clone the frame info because we need to store it as `prev_frame_info` at the end of the loop
                        let frame = self.frame_info().unwrap();

                        let pix = self.decode_raw()?;

                        // Use the new APNG post-processing function
                        apng_ctx.process_frame(&frame, &pix, &mut output)?;

                        // Create the frame from the fully composited output
                        let im_frame = Frame::from_u8(
                            &output,
                            colorspace,
                            usize::from(frame.delay_num),
                            usize::from(frame.delay_denom),
                        );
                        output_frames.push(im_frame);
                    }
                }
                BitType::U16 => {
                    let mut output = vec![0; buffer_size];

                    let mut apng_ctx = ApngContext::<u16>::new(self.info().unwrap(), colorspace);

                    while self.more_frames() {
                        self.decode_headers()?;

                        let frame = self.frame_info().unwrap();

                        if let DecodingResult::U16(pix) = self.decode()? {
                            apng_ctx.process_frame(&frame, &pix, &mut output)?;
                            // Create the frame from the fully composited output
                            let im_frame = Frame::from_u16(
                                &output,
                                colorspace,
                                usize::from(frame.delay_num),
                                usize::from(frame.delay_denom),
                            );
                            output_frames.push(im_frame);
                        } else {
                            unreachable!("Invalid image state, please report");
                        }
                    }
                }
                _ => unreachable!(),
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
                _ => unreachable!(),
            };
            // metadata
            image.metadata = metadata;

            Ok(image)
        }
    }
    fn dimensions(&self) -> Option<(usize, usize)> {
        self.dimensions()
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace().unwrap()
    }

    fn name(&self) -> &'static str {
        "PNG Decoder"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        self.decode_headers()
            .map_err(<error::PngDecodeErrors as Into<ImageErrors>>::into)?;

        let (width, height) = self.dimensions().unwrap();
        let depth = self.depth().unwrap();

        let info = self.info().unwrap();
        // 1. Map the cICP transfer function
        let transfer_curve = info
            .cicp_info
            .as_ref()
            .map(|cicp| match cicp.transfer_function {
                1 | 13 => ColorCharacteristics::sRGB,
                8 => ColorCharacteristics::Linear,
                16 => ColorCharacteristics::PQ,
                18 => ColorCharacteristics::HLG,
                v => ColorCharacteristics::Unknown(v),
            });

        // 2. Map the cHRM chunk to ColorPrimaries struct
        let color_primaries = info.chrm_info.as_ref().map(|chrm| ColorPrimaries {
            red: SingleColorPrimary {
                x: chrm.red_x as f64 / 100_000.0,
                y: chrm.red_y as f64 / 100_000.0,
                z: 0.0,
            },
            green: SingleColorPrimary {
                x: chrm.green_x as f64 / 100_000.0,
                y: chrm.green_y as f64 / 100_000.0,
                z: 0.0,
            },
            blue: SingleColorPrimary {
                x: chrm.blue_x as f64 / 100_000.0,
                y: chrm.blue_y as f64 / 100_000.0,
                z: 0.0,
            },
            white_point: SingleColorPrimary {
                x: chrm.white_point_x as f64 / 100_000.0,
                y: chrm.white_point_y as f64 / 100_000.0,
                z: 0.0,
            },
        });

        // 3. Extract the Brightness limits
        let max_cll = info.clli_info.as_ref().map(|clli| clli.max_cll);
        let mut metadata = ImageMetadata {
            format: Some(ImageFormat::PNG),
            colorspace: self.colorspace().unwrap(),
            depth,
            width,
            height,
            color_trc: transfer_curve,
            color_primaries,
            max_cll,
            color_standard: info.cicp_info.as_ref().map(|c| c.color_primaries),
            default_gamma: self.info().unwrap().gamma,
            ..Default::default()
        };
        #[cfg(feature = "metadata")]
        {
            let info = self.info().unwrap();
            // see if we have an exif chunk
            if let Some(exif) = &info.exif {
                metadata.parse_raw_exif(exif)
            }
        }
        // load icc
        if let Some(icc) = &self.info().unwrap().icc_profile {
            metadata.set_icc_chunk(icc.to_owned());
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
    options: Option<EncoderOptions>,
}

impl PngEncoder {
    pub fn new() -> PngEncoder {
        PngEncoder::default()
    }
    pub fn new_with_options(options: EncoderOptions) -> PngEncoder {
        PngEncoder {
            options: Some(options),
        }
    }
}

impl EncoderTrait for PngEncoder {
    fn name(&self) -> &'static str {
        "PNG encoder"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T,
    ) -> Result<usize, ImageErrors> {
        let options = create_options_for_encoder(self.options, image);

        let frames = image.frames_ref();
        if frames.is_empty() {
            return Err(ImageErrors::EncodeErrors(ImageEncodeErrors(
                "No frame".to_string(),
            )));
        }
        let frame = &image
            .frames_ref()
            .first()
            .unwrap()
            .u16_to_u8_endian(Endianness::Big)?;

        let mut encoder = zune_png::PngEncoder::new(frame, options);

        #[allow(unused_mut)]
        let mut buf: Cursor<Vec<u8>> = std::io::Cursor::new(vec![]);

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
        encoder
            .encode(sink)
            .map_err(|e| ImageErrors::EncodeErrors(ImageEncodeErrors(format!("{:?}", e))))
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &[
            ColorSpace::Luma,
            ColorSpace::LumaA,
            ColorSpace::RGB,
            ColorSpace::RGBA,
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
            _ => BitDepth::Eight,
        }
    }
    fn set_options(&mut self, opts: EncoderOptions) {
        self.options = Some(opts)
    }
}

impl<T> DecodeInto for PngDecoder<T>
where
    T: ZByteReaderTrait,
{
    type BufferType = u8;

    fn decode_into(&mut self, buffer: &mut [Self::BufferType]) -> Result<(), ImageErrors> {
        self.decode_into(buffer)
            .map_err(<PngDecodeErrors as Into<ImageErrors>>::into)?;

        Ok(())
    }

    fn decode_output_buffer_size(&mut self) -> Result<usize, ImageErrors> {
        self.decode_headers()
            .map_err(<PngDecodeErrors as Into<ImageErrors>>::into)?;

        // unwrap is okay because we successfully decoded image headers
        Ok(self.output_buffer_size().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;
    use zune_png::PngDecoder;

    use crate::codecs::png::PngEncoder;
    use crate::codecs::ImageFormat;
    use crate::image::Image;
    use crate::traits::DecodeInto;

    fn create_png() -> Vec<u8> {
        let encoder = PngEncoder::new();
        let image = Image::fill(10_u8, ColorSpace::RGB, 100, 100);
        image.write_to_vec(ImageFormat::PNG).unwrap()
    }
    #[test]
    fn test_png_decode_into() {
        let mut output = vec![0; 100 * 100 * 3];
        let img = create_png();
        let mut decoder = PngDecoder::new(ZCursor::new(&img));
        decoder.decode_into(&mut output).unwrap();
    }
}
