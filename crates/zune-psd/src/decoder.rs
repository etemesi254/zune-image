/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! A simple PSD reader.
//!
//! This crate features a simple and performant PSD reader
//! based on STB implementation.
//!
//! It currently does not support a lot of spec details.
//! Only extracting the image without respecting blend layers
//! and masks but such functionality will be added with time
//!
//!
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::log::trace;
use zune_core::options::DecoderOptions;
use zune_core::result::DecodingResult;

use crate::constants::{ColorModes, CompressionMethod, PSD_IDENTIFIER_BE};
use crate::errors::PSDDecodeErrors;

/// A simple Photoshop PSD reader.
///
/// This currently doesn't support layer flattening
/// but it's useful enough in that we can extract images
/// from it.
///
/// Further work will go onto adding a renderer that flattens
/// image pixels. But for now this is a good basis.
pub struct PSDDecoder<T>
where
    T: ZByteReaderTrait
{
    width:          usize,
    height:         usize,
    decoded_header: bool,
    stream:         ZReader<T>,
    options:        DecoderOptions,
    depth:          BitDepth,
    color_type:     Option<ColorModes>,
    compression:    CompressionMethod,
    channel_count:  usize
}

impl<T> PSDDecoder<T>
where
    T: ZByteReaderTrait
{
    /// Create a new decoder that reads a photoshop encoded file
    /// from `T` and returns pixels
    ///
    /// # Arguments
    /// - data: Data source, it has to implement the `ZReaderTrait
    pub fn new(data: T) -> PSDDecoder<T> {
        Self::new_with_options(data, DecoderOptions::default())
    }

    /// Creates a new decoder with options that influence decoding routines
    ///
    /// # Arguments
    /// - data: Data source
    /// - options: Custom options for the decoder
    pub fn new_with_options(data: T, options: DecoderOptions) -> PSDDecoder<T> {
        PSDDecoder {
            width: 0,
            height: 0,
            decoded_header: false,
            stream: ZReader::new(data),
            options,
            depth: BitDepth::Eight,
            color_type: None,
            compression: CompressionMethod::NoCompression,
            channel_count: 0
        }
    }

    /// Decode headers from the encoded image
    ///
    /// This confirms whether the image is a photoshop image and extracts
    /// relevant information from the image including width,height and extra information.
    ///
    pub fn decode_headers(&mut self) -> Result<(), PSDDecodeErrors> {
        if self.decoded_header {
            return Ok(());
        }
        // Check identifier
        let magic = self.stream.get_u32_be_err()?;

        if magic != PSD_IDENTIFIER_BE {
            return Err(PSDDecodeErrors::WrongMagicBytes(magic));
        }

        //  file version
        let version = self.stream.get_u16_be_err()?;

        if version != 1 {
            return Err(PSDDecodeErrors::UnsupportedFileType(version));
        }
        // Skip 6 reserved bytes
        self.stream.skip(6)?;
        // Read the number of channels (R, G, B, A, etc).
        let channel_count = self.stream.get_u16_be_err()?;

        if channel_count > 4 {
            return Err(PSDDecodeErrors::UnsupportedChannelCount(channel_count));
        }

        self.channel_count = usize::from(channel_count);

        let height = self.stream.get_u32_be_err()? as usize;
        let width = self.stream.get_u32_be_err()? as usize;

        if width > self.options.max_width() {
            return Err(PSDDecodeErrors::LargeDimensions(
                self.options.max_width(),
                width
            ));
        }

        if height > self.options.max_height() {
            return Err(PSDDecodeErrors::LargeDimensions(
                self.options.max_height(),
                height
            ));
        }

        self.width = width;
        self.height = height;

        if self.width == 0 || self.height == 0 || self.channel_count == 0 {
            return Err(PSDDecodeErrors::ZeroDimensions);
        }

        let depth = self.stream.get_u16_be_err()?;

        if depth != 8 && depth != 16 {
            return Err(PSDDecodeErrors::UnsupportedBitDepth(depth));
        }
        let im_depth = match depth {
            8 => BitDepth::Eight,
            16 => BitDepth::Sixteen,
            _ => unreachable!()
        };

        self.depth = im_depth;

        let color_mode = self.stream.get_u16_be_err()?;

        let color_enum = ColorModes::from_int(color_mode);

        if let Some(color) = color_enum {
            if !matches!(
                color,
                ColorModes::RGB | ColorModes::Grayscale | ColorModes::CYMK
            ) {
                return Err(PSDDecodeErrors::UnsupportedColorFormat(color_enum));
            }
        } else {
            return Err(PSDDecodeErrors::Generic("Unknown color mode"));
        }
        self.color_type = color_enum;

        // skip mode data
        let bytes = self.stream.get_u32_be_err()? as usize;
        self.stream.skip(bytes)?;

        // skip image resources
        let bytes = self.stream.get_u32_be_err()? as usize;
        self.stream.skip(bytes)?;

        // skip reserved data
        let bytes = self.stream.get_u32_be_err()? as usize;
        self.stream.skip(bytes)?;

        // find out if data is compressed
        let compression = self.stream.get_u16_be_err()?;

        if compression > 1 {
            return Err(PSDDecodeErrors::UnknownCompression);
        }
        if self.color_type == Some(ColorModes::Grayscale) {
            // PSD may have grayscale images with more than one
            // channel and will specify channel_count as 3.
            // So let's fix that here
            self.channel_count = 1;
        }

        self.compression = CompressionMethod::from_int(compression).unwrap();

        self.decoded_header = true;

        trace!("Image width:{}", self.width);
        trace!("Image height:{}", self.height);
        trace!("Channels: {}", self.channel_count);
        trace!("Bit depth : {:?}", self.depth);

        Ok(())
    }

    /// Decode an image to bytes without regard to depth or endianness
    ///
    /// # Returns
    /// Ok(bytes):  Raw bytes of the image
    /// Err(E): An error if it occurred during decoding
    pub fn decode_raw(&mut self) -> Result<Vec<u8>, PSDDecodeErrors> {
        if !self.decoded_header {
            self.decode_headers()?;
        }

        let pixel_count = self.width * self.height;

        let mut result = match (self.compression, self.depth) {
            (CompressionMethod::RLE, BitDepth::Eight) => {
                // RLE
                // Loop until you get the number of unpacked bytes you are expecting:
                //     Read the next source byte into n.
                //     If n is between 0 and 127 inclusive, copy the next n+1 bytes
                //     literally. Else if n is between -127 and -1 inclusive, copy the next
                //     byte -n+1 times. Else if n is 128, noop.
                // Endloop

                // The RLE-compressed data is preceded by a 2-byte data count for each row
                // in the data, which we're going to just skip.
                let skipped = self.height * self.channel_count * 2;
                self.stream.skip(skipped)?;

                let mut out_channel = vec![0; pixel_count * self.channel_count + 10];

                for channel in 0..self.channel_count {
                    let pixel_count = self.width * self.height;
                    self.psd_decode_rle(pixel_count, &mut out_channel[channel..])?;
                }

                out_channel.truncate(pixel_count * self.channel_count);

                out_channel
            }
            (CompressionMethod::NoCompression, BitDepth::Eight) => {
                // We're at the raw image data.  It's each channel in order (Red, Green,
                // Blue, Alpha, ...) where each channel consists of an 8-bit
                // value for each pixel in the image.

                // Read the data by channel.

                let mut out_channel = vec![0; self.width * self.height * self.channel_count + 10];
                let pixel_count = self.width * self.height;

                // // check we have enough data
                // if !self.stream.has(pixel_count * self.channel_count) {
                //     return Err(PSDDecodeErrors::Generic("Incomplete bitstream"));
                // }

                for channel in 0..self.channel_count {
                    let mut i = channel;

                    while i < pixel_count {
                        out_channel[i] = self.stream.read_u8_err()?;
                        i += self.channel_count;
                    }
                }

                out_channel.truncate(pixel_count * self.channel_count);
                out_channel
            }

            (CompressionMethod::NoCompression, BitDepth::Sixteen) => {
                // We're at the raw image data.  It's each channel in order (Red, Green,
                // Blue, Alpha, ...) where each channel consists of an 8-bit
                // value for each pixel in the image.

                // Read the data by channel.

                // size of a single channel
                let channel_dimensions = self.width * self.height;

                let mut out_channel = vec![0; 2 * (channel_dimensions * self.channel_count + 10)];

                let pixel_count = channel_dimensions * 2;

                // check we have enough data
                // if !self.stream.has(pixel_count * self.channel_count) {
                //     return Err(PSDDecodeErrors::Generic("Incomplete bitstream"));
                // }

                // iterate per channel
                for channel in 0..self.channel_count {
                    let i = channel * 2;
                    let out_chunks = out_channel[i..].chunks_exact_mut(self.channel_count * 2);

                    // iterate only taking the image dimensions
                    for out in out_chunks.take(channel_dimensions) {
                        let value = self.stream.get_u16_be_err()?;

                        out[..2].copy_from_slice(&value.to_ne_bytes());
                    }
                }

                out_channel.truncate(pixel_count * self.channel_count);
                out_channel
            }
            _ => return Err(PSDDecodeErrors::Generic("Not implemented or Unknown"))
        };
        // remove white matte from psd
        if self.channel_count >= 4 {
            match self.depth {
                BitDepth::Sixteen => {
                    for pixel in result.chunks_exact_mut(8) {
                        let px3 = u16::from_be_bytes(pixel[6..8].try_into().unwrap());
                        if px3 != 0 && px3 != 65535 {
                            let px0 = u16::from_be_bytes(pixel[0..2].try_into().unwrap());
                            let px1 = u16::from_be_bytes(pixel[2..4].try_into().unwrap());
                            let px2 = u16::from_be_bytes(pixel[4..6].try_into().unwrap());

                            let a = f32::from(px3) / 65535.0;
                            let ra = 1.0 / a;
                            let inv_a = 65535.0 * (1.0 - ra);

                            let x = (f32::from(px0) * ra + inv_a) as u16;
                            let y = (f32::from(px1) * ra + inv_a) as u16;
                            let z = (f32::from(px2) * ra + inv_a) as u16;

                            pixel[0..2].copy_from_slice(&x.to_ne_bytes());
                            pixel[2..4].copy_from_slice(&y.to_ne_bytes());
                            pixel[4..6].copy_from_slice(&z.to_ne_bytes());
                        }
                    }
                }
                BitDepth::Eight => {
                    for pixel in result.chunks_exact_mut(4) {
                        if pixel[3] != 0 && pixel[3] != 255 {
                            let a = f32::from(pixel[3]) / 255.0;
                            let ra = 1.0 / a;
                            let inv_a = 255.0 * (1.0 - ra);
                            pixel[0] = (f32::from(pixel[0]) * ra + inv_a) as u8;
                            pixel[1] = (f32::from(pixel[1]) * ra + inv_a) as u8;
                            pixel[2] = (f32::from(pixel[2]) * ra + inv_a) as u8;
                        }
                    }
                }
                _ => unreachable!()
            }
        }
        Ok(result)
    }
    /// Decode a PSD file extracting the image only
    ///
    /// Currently this does it without respect to  layers
    /// and such, only extracting the PSD image, hence might not be the
    /// most useful one.
    ///
    pub fn decode(&mut self) -> Result<DecodingResult, PSDDecodeErrors> {
        let raw = self.decode_raw()?;

        if self.depth == BitDepth::Eight {
            return Ok(DecodingResult::U8(raw));
        }
        if self.depth == BitDepth::Sixteen {
            // https://github.com/etemesi254/zune-image/issues/36
            let new_array: Vec<u16> = raw
                .chunks_exact(2)
                .map(|chunk| {
                    let value: [u8; 2] = chunk.try_into().unwrap();
                    u16::from_be_bytes(value)
                })
                .collect();

            return Ok(DecodingResult::U16(new_array));
        }

        Err(PSDDecodeErrors::Generic("Not implemented"))
    }

    fn psd_decode_rle(
        &mut self, pixel_count: usize, buffer: &mut [u8]
    ) -> Result<(), PSDDecodeErrors> {
        let mut count = 0;
        let mut nleft = pixel_count - count;

        let mut position = 0;

        while nleft > 0 {
            let mut len = usize::from(self.stream.read_u8());

            match len.cmp(&128) {
                Ordering::Less => {
                    // copy next len+1 bytes literally
                    len += 1;
                    if len > nleft {
                        return Err(PSDDecodeErrors::BadRLE);
                    }
                    count += len;

                    if position + (self.channel_count * len) > buffer.len() {
                        return Err(PSDDecodeErrors::BadRLE);
                    }

                    while len > 0 {
                        buffer[position] = self.stream.read_u8();
                        position += self.channel_count;
                        len -= 1;
                    }
                }
                Ordering::Equal => (),
                Ordering::Greater => {
                    // Next -len+1 bytes in the dest are replicated from next source byte.
                    // (Interpret len as a negative 8-bit int.)
                    len = 257_usize.wrapping_sub(len) & 255;

                    if len > nleft {
                        return Err(PSDDecodeErrors::BadRLE);
                    }
                    count += len;
                    let val = self.stream.read_u8();

                    if position + (self.channel_count * len) > buffer.len() {
                        return Err(PSDDecodeErrors::BadRLE);
                    }

                    while len > 0 {
                        buffer[position] = val;
                        position += self.channel_count;
                        len -= 1;
                    }
                }
            }

            nleft = pixel_count - count;
        }
        Ok(())
    }

    /// Get image bit depth or None if the headers haven't been decoded
    pub const fn bit_depth(&self) -> Option<BitDepth> {
        if self.decoded_header {
            return Some(self.depth);
        }
        None
    }

    /// Get image width and height respectively or None if the
    /// headers haven't been decoded
    pub fn dimensions(&self) -> Option<(usize, usize)> {
        if self.decoded_header {
            return Some((self.width, self.height));
        }
        None
    }
    /// Get image colorspace or None if the
    /// image header hasn't been decoded
    pub fn colorspace(&self) -> Option<ColorSpace> {
        if let Some(color) = self.color_type {
            if color == ColorModes::RGB {
                return if self.channel_count == 4 {
                    Some(ColorSpace::RGBA)
                } else {
                    Some(ColorSpace::RGB)
                };
            } else if color == ColorModes::Grayscale {
                return if self.channel_count == 1 {
                    Some(ColorSpace::Luma)
                } else if self.channel_count == 2 {
                    Some(ColorSpace::LumaA)
                } else {
                    None
                };
            }
            if color == ColorModes::CYMK {
                return Some(ColorSpace::CMYK);
            }
        }
        None
    }
}
