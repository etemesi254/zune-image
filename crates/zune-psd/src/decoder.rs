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
    T: ZByteReaderTrait,
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
                width,
            ));
        }

        if height > self.options.max_height() {
            return Err(PSDDecodeErrors::LargeDimensions(
                self.options.max_height(),
                height,
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
            _ => unreachable!(),
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

    /// Decodes the image into a raw byte buffer.
    ///
    /// This is a low-level decoding function that returns the underlying pixel
    /// data as a flat `Vec<u8>` without enforcing a specific interpretation of
    /// bit depth or endianness beyond what is produced internally.
    ///
    /// Internally, this allocates a buffer of size [`required_len`] and delegates
    /// decoding to [`decode_into`].
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<u8>)` containing the decoded pixel data
    /// - `Err(_)` if decoding fails
    ///
    /// The returned buffer:
    /// - Has length exactly equal to [`required_len`]
    /// - Is interleaved by channel (e.g. RGBA RGBA ...)
    /// - Uses:
    ///   - 1 byte per channel for 8-bit images
    ///   - 2 bytes per channel for 16-bit images (native endian)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Header decoding fails
    /// - The compression method or bit depth is unsupported
    /// - The underlying stream is invalid or truncated
    ///
    /// # Notes
    ///
    /// - This function performs a heap allocation. For allocation-free decoding,
    ///   use [`decode_into`].
    /// - If an alpha channel is present, white matte is removed during decoding.
    /// - Headers are decoded automatically if not already processed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_psd::errors::PSDDecodeErrors;
    /// use zune_psd::PSDDecoder;
    /// let mut decoder = PSDDecoder::new(ZCursor::new(vec![0]));
    /// let raw = decoder.decode_raw()?;
    /// println!("decoded {} bytes", raw.len());
    ///
    /// Ok::<(),PSDDecodeErrors>(())
    /// ```
    pub fn decode_raw(&mut self) -> Result<Vec<u8>, PSDDecodeErrors> {
        if !self.decoded_header {
            self.decode_headers()?;
        }

        let required_length = self.required_len()?;
        let mut out = vec![0; required_length];
        let new_length = self.decode_into(&mut out)?;
        debug_assert!(new_length == required_length);
        Ok(out)
    }
    /// Returns the exact number of bytes required to hold the decoded image data.
    ///
    /// This value can be used to preallocate a buffer for [`decode_into`].
    ///
    /// The size is computed as:
    /// `width * height * channel_count * bytes_per_sample`
    ///
    /// Where:
    /// - `bytes_per_sample` is:
    ///   - `1` for 8-bit images
    ///   - `2` for 16-bit images
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The PSD headers have not been decoded yet
    /// - The bit depth is unsupported
    /// - The computed size overflows `usize`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use zune_psd::PSDDecoder;
    /// use zune_psd::errors::PSDDecodeErrors;
    /// use zune_core::bytestream::ZCursor;
    ///  let mut decoder = PSDDecoder::new(ZCursor::new(vec![0]));
    /// let len = decoder.required_len()?;
    /// let mut buffer = vec![0u8; len];
    /// decoder.decode_into(&mut buffer)?;
    ///
    /// Ok::<(),PSDDecodeErrors>(())
    /// ```
    ///
    /// # Notes
    ///
    /// - The returned size is the exact number of bytes written by [`decode_into`]
    ///   on success.
    /// - The output layout is interleaved by channel (e.g. RGBA RGBA ...).
    /// - No padding or alignment bytes are included.
    pub fn required_len(&self) -> Result<usize, PSDDecodeErrors> {
        if !self.decoded_header {
            return Err(PSDDecodeErrors::Generic("Headers not decoded"));
        }

        let bytes_per_sample = match self.depth {
            BitDepth::Eight => 1usize,
            BitDepth::Sixteen => 2usize,
            _ => return Err(PSDDecodeErrors::Generic("Unsupported bit depth")),
        };

        let pixel_count = self.width
            .checked_mul(self.height)
            .ok_or(PSDDecodeErrors::Generic("Dimension overflow"))?;

        let total = pixel_count
            .checked_mul(self.channel_count)
            .and_then(|v| v.checked_mul(bytes_per_sample))
            .ok_or(PSDDecodeErrors::Generic("Size overflow"))?;

        Ok(total)
    }
    /// Decodes the PSD image into a caller-provided buffer.
    ///
    /// This function writes decoded pixel data into `output` without allocating.
    /// The caller is responsible for providing a buffer large enough to hold
    /// the result (see [`required_len`]).
    ///
    /// # Parameters
    ///
    /// - `output`: Destination buffer for decoded pixel data.
    ///
    /// # Returns
    ///
    /// The number of bytes written into `output` on success.
    /// This will always equal [`required_len`] if the call succeeds.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Header decoding fails
    /// - The output buffer is too small
    /// - The compression method or bit depth is unsupported
    /// - The underlying stream is invalid or truncated
    ///
    /// # Buffer Requirements
    ///
    /// The buffer must be at least:
    /// `width * height * channel_count * bytes_per_sample` bytes long.
    ///
    /// You can obtain this size via [`required_len`].
    ///
    /// # Output Format
    ///
    /// - Pixel data is interleaved by channel (e.g. RGBA RGBA ...).
    /// - 8-bit images use 1 byte per channel.
    /// - 16-bit images use 2 bytes per channel (native endian).
    /// - If an alpha channel is present, white matte is removed.
    ///
    /// # Notes
    ///
    /// - This function does not allocate.
    /// - Any extra capacity in `output` beyond the required size is ignored.
    /// - The function assumes the stream is positioned at the start of image data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use zune_psd::PSDDecoder;
    /// use zune_psd::errors::PSDDecodeErrors;
    /// use zune_core::bytestream::ZCursor;
    /// let mut decoder = PSDDecoder::new(ZCursor::new(vec![1]));
    /// let len = decoder.required_len()?;
    /// let mut buffer = vec![0u8; len];
    ///
    /// let written = decoder.decode_into(&mut buffer)?;
    /// assert_eq!(written, len);
    /// Ok::<(),PSDDecodeErrors>(())
    /// ```
    pub fn decode_into(&mut self, output: &mut [u8]) -> Result<usize, PSDDecodeErrors> {
        if !self.decoded_header {
            self.decode_headers()?;
        }

        let pixel_count = self.width * self.height;
        let bytes_per_sample = match self.depth {
            BitDepth::Eight => 1,
            BitDepth::Sixteen => 2,
            _ => return Err(PSDDecodeErrors::Generic("Unsupported bit depth")),
        };

        let required_len = pixel_count * self.channel_count * bytes_per_sample;

        if output.len() < required_len {
            return Err(PSDDecodeErrors::Generic("Output buffer too small"));
        }

        let out = &mut output[..required_len];

        match (self.compression, self.depth) {
            (CompressionMethod::RLE, BitDepth::Eight) => {
                let skipped = self.height * self.channel_count * 2;
                self.stream.skip(skipped)?;

                for channel in 0..self.channel_count {
                    self.psd_decode_rle(pixel_count, &mut out[channel..])?;
                }
            }

            (CompressionMethod::NoCompression, BitDepth::Eight) => {
                for channel in 0..self.channel_count {
                    let mut i = channel;

                    while i < pixel_count {
                        out[i] = self.stream.read_u8_err()?;
                        i += self.channel_count;
                    }
                }
            }

            (CompressionMethod::NoCompression, BitDepth::Sixteen) => {
                let channel_pixels = pixel_count;

                for channel in 0..self.channel_count {
                    let i = channel * 2;
                    let chunks = out[i..].chunks_exact_mut(self.channel_count * 2);

                    for chunk in chunks.take(channel_pixels) {
                        let value = self.stream.get_u16_be_err()?;
                        chunk[..2].copy_from_slice(&value.to_ne_bytes());
                    }
                }
            }

            _ => return Err(PSDDecodeErrors::Generic("Not implemented or Unknown")),
        }

        // Remove white matte
        if self.channel_count >= 4 {
            match self.depth {
                BitDepth::Eight => {
                    for pixel in out.chunks_exact_mut(4) {
                        if pixel[3] != 0 && pixel[3] != 255 {
                            let a = pixel[3] as f32 / 255.0;
                            let ra = 1.0 / a;
                            let inv_a = 255.0 * (1.0 - ra);

                            pixel[0] = (pixel[0] as f32 * ra + inv_a) as u8;
                            pixel[1] = (pixel[1] as f32 * ra + inv_a) as u8;
                            pixel[2] = (pixel[2] as f32 * ra + inv_a) as u8;
                        }
                    }
                }

                BitDepth::Sixteen => {
                    for pixel in out.chunks_exact_mut(8) {
                        let a = u16::from_be_bytes(pixel[6..8].try_into().unwrap());

                        if a != 0 && a != 65535 {
                            let r = u16::from_be_bytes(pixel[0..2].try_into().unwrap());
                            let g = u16::from_be_bytes(pixel[2..4].try_into().unwrap());
                            let b = u16::from_be_bytes(pixel[4..6].try_into().unwrap());

                            let af = a as f32 / 65535.0;
                            let ra = 1.0 / af;
                            let inv_a = 65535.0 * (1.0 - ra);

                            let r = (r as f32 * ra + inv_a) as u16;
                            let g = (g as f32 * ra + inv_a) as u16;
                            let b = (b as f32 * ra + inv_a) as u16;

                            pixel[0..2].copy_from_slice(&r.to_ne_bytes());
                            pixel[2..4].copy_from_slice(&g.to_ne_bytes());
                            pixel[4..6].copy_from_slice(&b.to_ne_bytes());
                        }
                    }
                }

                _ => unreachable!(),
            }
        }

        Ok(required_len)
    }
    /// Decodes the image into a typed pixel buffer.
    ///
    /// This is the high-level decoding API. It interprets the raw byte output
    /// from [`decode_raw`] and converts it into a typed representation based
    /// on the image bit depth.
    ///
    /// # Returns
    ///
    /// - `DecodingResult::U8(Vec<u8>)` for 8-bit images
    /// - `DecodingResult::U16(Vec<u16>)` for 16-bit images
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Decoding fails at the raw stage
    /// - The bit depth is unsupported
    ///
    /// # Output Format
    ///
    /// - Pixel data is interleaved by channel (e.g. RGBA RGBA ...)
    /// - For 16-bit images:
    ///   - Input bytes are interpreted as **big-endian**
    ///   - Values are converted into native `u16`
    ///
    /// # Notes
    ///
    /// - This function always allocates a new buffer.
    /// - For 16-bit images, this performs an additional conversion pass
    ///   from `Vec<u8>` to `Vec<u16>`.
    /// - If you need raw access or want to avoid the conversion cost,
    ///   use [`decode_raw`] or [`decode_into`] instead.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_psd::PSDDecoder;
    /// use zune_core::result::DecodingResult;
    /// use zune_psd::errors::PSDDecodeErrors;
    /// let mut decoder = PSDDecoder::new(ZCursor::new(vec![0]));
    /// match decoder.decode()? {
    ///     DecodingResult::U8(pixels) => {
    ///         println!("8-bit image: {} bytes", pixels.len());
    ///     }
    ///     DecodingResult::U16(pixels) => {
    ///         println!("16-bit image: {} samples", pixels.len());
    ///     }
    ///     _=>unreachable!()
    /// }
    ///Ok::<(),PSDDecodeErrors>(())
    /// ```
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
        &mut self, pixel_count: usize, buffer: &mut [u8],
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
