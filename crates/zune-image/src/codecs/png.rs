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

//! Represents a png image decoder and encoder

use std::borrow::Cow;
use std::io::{BufRead, Seek};

use png::chunk::ChunkType;
use png::{BitDepth as PngBitDepth, ColorType, Compression, Decoder, Encoder, Reader};
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteWriterTrait;
use zune_core::colorspace::ColorSpace;
use zune_core::log::{trace, warn};
use zune_core::options::{EncoderOptions, PngCompression};

use crate::codecs::{create_options_for_encoder, ImageFormat};
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::metadata::{AlphaState, ImageMetadata};
use crate::traits::{DecodeInto, DecoderTrait, EncoderTrait};

pub struct PngDecoder<T: BufRead + Seek> {
    inner: Reader<T>
}

impl<T: BufRead + Seek> PngDecoder<T> {
    pub fn new(r: T) -> Result<Self, ImageErrors> {
        let reader = Decoder::new(r)
            .read_info()
            .map_err(|e| ImageErrors::ImageDecodeErrors(e.to_string()))?;

        Ok(PngDecoder { inner: reader })
    }
}

impl<T: BufRead + Seek> DecoderTrait for PngDecoder<T> {
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let info = self.inner.info();
        let width = info.width as usize;
        let height = info.height as usize;
        let colorspace = png_color_type_to_colorspace(info.color_type);
        let depth = png_bit_depth_to_zune(info.bit_depth);

        let output_size = self
            .inner
            .output_buffer_size()
            .ok_or(ImageErrors::ImageDecodeErrors(
                "Output buffer overflowed".to_string()
            ))?;
        let mut raw_pixels = vec![0_u8; output_size];

        self.inner
            .next_frame(&mut raw_pixels)
            .map_err(|e| ImageErrors::ImageDecodeErrors(e.to_string()))?;

        let mut img = match depth {
            BitDepth::Sixteen => {
                // image-png gives 16-bit data as big-endian bytes; convert to u16
                let pixels_u16: Vec<u16> = raw_pixels
                    .chunks_exact(2)
                    .map(|b| u16::from_be_bytes([b[0], b[1]]))
                    .collect();
                Image::from_u16(&pixels_u16, width, height, colorspace)
            }
            _ => Image::from_u8(&raw_pixels, width, height, colorspace)
        };

        img.metadata.format = Some(ImageFormat::PNG);

        Ok(img)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        let info = self.inner.info();
        Some((info.width as usize, info.height as usize))
    }

    fn out_colorspace(&self) -> ColorSpace {
        png_color_type_to_colorspace(self.inner.info().color_type)
    }

    fn name(&self) -> &'static str {
        "PNG Decoder (image-png)"
    }

    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, ImageErrors> {
        let info = self.inner.info();
        let width = info.width as usize;
        let height = info.height as usize;
        let colorspace = png_color_type_to_colorspace(info.color_type);
        let depth = png_bit_depth_to_zune(info.bit_depth);

        let mut metadata = ImageMetadata {
            format: Some(ImageFormat::PNG),
            colorspace,
            depth,
            width,
            height,
            default_gamma: info.gama_chunk.map(|g| g.into_value()),
            ..Default::default()
        };

        if let Some(icc) = &info.icc_profile {
            metadata.set_icc_chunk(icc.to_vec());
        }

        Ok(Some(metadata))
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
        "PNG encoder (image-png)"
    }

    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, mut sink: T
    ) -> Result<usize, ImageErrors> {
        let options = create_options_for_encoder(self.options, image);

        let width = options.width() as u32;
        let height = options.height() as u32;
        let colorspace = options.colorspace();
        let bit_depth = options.depth();

        let color_type = zune_colorspace_to_png(colorspace).ok_or_else(|| {
            ImageErrors::EncodeErrors(ImgEncodeErrors::UnsupportedColorspace(
                colorspace,
                self.supported_colorspaces()
            ))
        })?;

        let mut output: Vec<u8> = Vec::new();

        {
            let mut encoder = Encoder::new(&mut output, width, height);
            encoder.set_color(color_type);

            let compression_level = match options.png_compression_level() {
                PngCompression::NoCompression => png::Compression::NoCompression,
                PngCompression::Fastest => png::Compression::Fastest,
                PngCompression::Fast => png::Compression::Fast,
                PngCompression::Balanced => png::Compression::Balanced,
                PngCompression::High => Compression::High
            };

            encoder.set_compression(compression_level);

            encoder.set_depth(match bit_depth {
                BitDepth::Sixteen => PngBitDepth::Sixteen,
                _ => PngBitDepth::Eight
            });

            let mut writer = encoder
                .write_header()
                .map_err(|e| ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(e.to_string())))?;

            let frame_data = &image.to_u8_be()[0];

            #[cfg(feature = "metadata")]
            {
                use exif::experimental::Writer;

                if !options.strip_metadata() {
                    if let Some(fields) = &image.metadata.exif {
                        let mut buf = std::io::Cursor::new(Vec::new());
                        let mut exif_writer = Writer::new();
                        for metadatum in fields {
                            exif_writer.push_field(metadatum);
                        }
                        let result = exif_writer.write(&mut buf, false);
                        if result.is_ok() {
                            writer
                                .write_chunk(ChunkType(*b"eXIf"), &buf.into_inner())
                                .map_err(|e| {
                                    ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(
                                        e.to_string()
                                    ))
                                })?;
                            trace!("Added eXIF chunk")
                        } else {
                            warn!("Writing exif failed {:?}", result);
                        }
                    }
                }
            }
            if !options.strip_metadata() {
                //todo:  CAE:Support ICC chunk, image-png has no support as of now

                if let Some(icc) = image.metadata().icc_chunk.as_ref() {
                    warn!("ICC chunk will not be saved in image");
                    //
                    // writer.write_chunk(ChunkType(*b"iCCP"), icc).map_err(|e| {
                    //     ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(e.to_string()))
                    // })?;
                    trace!("Added ICC chunk")
                }
            }

            writer
                .write_image_data(frame_data)
                .map_err(|e| ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(e.to_string())))?;
        }

        sink.write_all_bytes(output.as_ref())?;
        Ok(output.len())
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

impl<T> DecodeInto for PngDecoder<T>
where
    T: BufRead + Seek
{
    type BufferType = u8;

    fn decode_into(&mut self, buffer: &mut [Self::BufferType]) -> Result<(), ImageErrors> {
        self.inner
            .next_frame(buffer)
            .map_err(|e| ImageErrors::ImageDecodeErrors(e.to_string()))?;

        Ok(())
    }

    fn decode_output_buffer_size(&mut self) -> Result<usize, ImageErrors> {
        let output_size = self
            .inner
            .output_buffer_size()
            .ok_or(ImageErrors::ImageDecodeErrors(
                "Output buffer overflowed".to_string()
            ))?;
        Ok(output_size)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn png_color_type_to_colorspace(ct: ColorType) -> ColorSpace {
    match ct {
        ColorType::Grayscale => ColorSpace::Luma,
        ColorType::GrayscaleAlpha => ColorSpace::LumaA,
        ColorType::Rgb => ColorSpace::RGB,
        ColorType::Rgba => ColorSpace::RGBA,
        // Indexed PNG is expanded to RGB by image-png by default
        ColorType::Indexed => ColorSpace::RGB
    }
}

fn png_bit_depth_to_zune(bd: PngBitDepth) -> BitDepth {
    match bd {
        PngBitDepth::Sixteen => BitDepth::Sixteen,
        _ => BitDepth::Eight
    }
}

fn zune_colorspace_to_png(cs: ColorSpace) -> Option<ColorType> {
    match cs {
        ColorSpace::Luma => Some(ColorType::Grayscale),
        ColorSpace::LumaA => Some(ColorType::GrayscaleAlpha),
        ColorSpace::RGB => Some(ColorType::Rgb),
        ColorSpace::RGBA => Some(ColorType::Rgba),
        _ => None
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use zune_core::colorspace::ColorSpace;

    use crate::codecs::png::{PngDecoder, PngEncoder};
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
        let mut decoder = PngDecoder::new(Cursor::new(&img)).unwrap();
        decoder.decode_into(&mut output).unwrap();
    }
}
