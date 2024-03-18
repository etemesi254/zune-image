/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::iter::Iterator;
use core::option::Option::{self, *};
use core::result::Result::{self, *};

use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::log::trace;
use zune_core::options::DecoderOptions;

use crate::errors::HdrDecodeErrors;

/// A simple radiance HDR decoder
///
/// # Accessing metadata
///
/// Radiance files may contain metadata in it's headers as key value pairs,
/// we save the metadata in a hashmap and provide a way to inspect that metadata by exposing
/// the map as an API access method.
///
/// For sophisticated algorithms, they may use the metadata to further understand the data.
pub struct HdrDecoder<T: ZByteReaderTrait> {
    buf:             ZReader<T>,
    options:         DecoderOptions,
    metadata:        BTreeMap<String, String>,
    width:           usize,
    height:          usize,
    decoded_headers: bool
}

impl<T> HdrDecoder<T>
where
    T: ZByteReaderTrait
{
    /// Create a new HDR decoder
    ///
    /// # Arguments
    ///
    /// * `data`: Raw HDR file contents
    ///
    /// returns: HdrDecoder
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_hdr::HdrDecoder;
    /// // read hdr file to memory
    /// let file_data = std::io::BufReader::new(std::fs::File::open("sample.hdr").unwrap());
    /// let decoder = HdrDecoder::new(file_data);
    /// ```
    pub fn new(data: T) -> HdrDecoder<T> {
        Self::new_with_options(data, DecoderOptions::default())
    }

    /// Create a new HDR decoder with the specified options
    ///
    /// # Arguments
    ///
    /// * `data`: Raw HDR file contents already in memory
    /// * `options`: Decoder options that influence how decoding occurs
    ///
    /// returns: HdrDecoder
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufReader;
    /// use zune_core::options::DecoderOptions;
    /// use zune_hdr::HdrDecoder;
    /// // read hdr file to memory
    /// let file_data = std::fs::File::open("sample.hdr").unwrap();
    /// // set that the decoder does not decode images greater than
    /// // 50 px width
    /// let options = DecoderOptions::default().set_max_width(50);
    /// // use the options set
    /// let decoder = HdrDecoder::new_with_options(BufReader::new(file_data),options);
    /// ```
    pub fn new_with_options(data: T, options: DecoderOptions) -> HdrDecoder<T> {
        HdrDecoder {
            buf: ZReader::new(data),
            options,
            width: 0,
            height: 0,
            metadata: BTreeMap::new(),
            decoded_headers: false
        }
    }
    /// Get key value metadata found in the header
    ///
    ///
    /// In case the key or value contains non-valid UTF-8, the
    /// characters are replaced with [REPLACEMENT_CHARACTER](core::char::REPLACEMENT_CHARACTER)
    pub const fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }
    /// Decode headers for the HDR image
    ///
    /// The struct is modified in place and data can be
    /// extracted from appropriate getters.
    pub fn decode_headers(&mut self) -> Result<(), HdrDecodeErrors> {
        // maximum size for which we expect the buffer to be
        let mut max_header_size = vec![0; 1024];

        if self.decoded_headers {
            return Ok(());
        }
        self.get_buffer_until(b'\n', &mut max_header_size)?;

        if !(max_header_size.starts_with(b"#?RADIANCE\n")
            || max_header_size.starts_with(b"#?RGBE\n"))
        {
            return Err(HdrDecodeErrors::InvalidMagicBytes);
        }

        loop {
            let size = self.get_buffer_until(b'\n', &mut max_header_size)?;
            if max_header_size.starts_with(b"#")
            // comment
            {
                continue;
            }
            if max_header_size[..size].contains(&b'=') {
                // key value, it should be lossy to avoid failure when the key is not valid
                // utf-8, we throw garbage to the dictionary if the image is garbage
                let keys_and_values = String::from_utf8_lossy(&max_header_size[..size]);

                let mut keys_and_values_split = keys_and_values.trim().split('=');
                let key = keys_and_values_split.next().unwrap().trim().to_string();
                let value = keys_and_values_split.next().unwrap().trim().to_string();
                self.metadata.insert(key, value);
            }

            if size == 0 || max_header_size[0] == b'\n' {
                trace!("Metadata: {:?}", self.metadata);
                break;
            }
        }
        let header_size = self.get_buffer_until(b' ', &mut max_header_size)?;

        let first_type = String::from_utf8_lossy(&max_header_size[..header_size])
            .trim()
            .to_string();

        let header_size = self.get_buffer_until(b' ', &mut max_header_size)?;

        let coords1 = String::from_utf8_lossy(&max_header_size[..header_size])
            .trim()
            .to_string();

        let header_size = self.get_buffer_until(b' ', &mut max_header_size)?;

        let second_type = String::from_utf8_lossy(&max_header_size[..header_size])
            .trim()
            .to_string();

        let header_size = self.get_buffer_until(b'\n', &mut max_header_size)?;

        let coords2 = String::from_utf8_lossy(&max_header_size[..header_size])
            .trim()
            .to_string();

        match (first_type.as_str(), second_type.as_str()) {
            ("-Y", "+X") => {
                self.height = coords1.parse::<usize>()?;
                self.width = coords2.parse::<usize>()?;
            }
            ("+X", "-Y") => {
                self.height = coords2.parse::<usize>()?;
                self.width = coords1.parse::<usize>()?;
            }
            (_, _) => {
                return Err(HdrDecodeErrors::UnsupportedOrientation(
                    first_type,
                    second_type
                ));
            }
        }
        if self.height > self.options.max_height() {
            return Err(HdrDecodeErrors::TooLargeDimensions(
                "height",
                self.options.max_height(),
                self.height
            ));
        }

        if self.width > self.options.max_width() {
            return Err(HdrDecodeErrors::TooLargeDimensions(
                "width",
                self.options.max_width(),
                self.width
            ));
        }

        trace!("Width: {}", self.width);
        trace!("Height: {}", self.height);

        self.decoded_headers = true;

        Ok(())
    }

    /// Get image dimensions as a tuple of width and height
    /// or `None` if the image hasn't been decoded.
    ///
    /// # Returns
    /// - `Some(width,height)`: Image dimensions
    /// -  None : The image headers haven't been decoded
    pub const fn dimensions(&self) -> Option<(usize, usize)> {
        if self.decoded_headers {
            Some((self.width, self.height))
        } else {
            None
        }
    }

    /// Return the input colorspace of the image
    ///
    /// # Returns
    /// -`Some(Colorspace)`: Input colorspace
    /// - None : Indicates the headers weren't decoded
    pub fn get_colorspace(&self) -> Option<ColorSpace> {
        if self.decoded_headers {
            Some(ColorSpace::RGB)
        } else {
            None
        }
    }

    /// Decode HDR file return a vector containing decoded
    /// coefficients
    ///
    /// # Returns
    /// - `Ok(Vec<f32>)`: The actual decoded coefficients
    /// - `Err(HdrDecodeErrors)`: Indicates an unrecoverable
    ///  error occurred during decoding.
    pub fn decode(&mut self) -> Result<Vec<f32>, HdrDecodeErrors> {
        self.decode_headers()?;
        let mut buffer = vec![0.0f32; self.width * self.height * 3];

        self.decode_into(&mut buffer)?;

        Ok(buffer)
    }
    /// Return the number of bytes required to hold a decoded image frame
    /// decoded using the given input transformations
    ///
    /// # Returns
    ///  - `Some(usize)`: Minimum size for a buffer needed to decode the image
    ///  - `None`: Indicates the image headers were not decoded or
    /// `width*height*colorspace` calculation  overflows a usize
    ///
    pub fn output_buffer_size(&self) -> Option<usize> {
        if self.decoded_headers {
            Some(self.width.checked_mul(self.height)?.checked_mul(3)?)
        } else {
            None
        }
    }

    /// Decode into a pre-allocated buffer
    ///
    /// It is an error if the buffer size is smaller than
    /// [`output_buffer_size()`](Self::output_buffer_size)
    ///
    /// If the buffer is bigger than expected, we ignore the end padding bytes
    ///
    /// # Example
    ///
    /// - Read  headers and then alloc a buffer big enough to hold the image
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_hdr::HdrDecoder;
    /// let mut decoder = HdrDecoder::new(ZCursor::new(&[]));
    /// // before we get output, we must decode the headers to get width
    /// // height, and input colorspace
    /// decoder.decode_headers().unwrap();
    ///
    /// let mut out = vec![0.0;decoder.output_buffer_size().unwrap()];
    /// // write into out
    /// decoder.decode_into(&mut out).unwrap();
    /// ```
    pub fn decode_into(&mut self, buffer: &mut [f32]) -> Result<(), HdrDecodeErrors> {
        if !self.decoded_headers {
            self.decode_headers()?;
        }

        let output_size = self.output_buffer_size().unwrap();

        if buffer.len() < output_size {
            return Err(HdrDecodeErrors::TooSmallOutputArray(
                output_size,
                buffer.len()
            ));
        }

        // single width scanline
        let mut scanline = vec![0_u8; self.width * 4]; // R,G,B,E

        let output_scanline_size = self.width * 3; // RGB, * width gives us size of one scanline

        // read flat data
        for out_scanline in buffer
            .chunks_exact_mut(output_scanline_size)
            .take(self.height)
        {
            if self.width < 8 || self.width > 0x7fff {
                self.decompress(&mut scanline, self.width as i32, 0)?;
                convert_scanline(&scanline, out_scanline);
                continue;
            }

            let mut i = self.buf.read_u8();

            if i != 2 {
                // undo byte read
                self.buf.rewind(1)?;

                self.decompress(&mut scanline, self.width as i32, 0)?;
                convert_scanline(&scanline, out_scanline);
                continue;
            }

            scanline[1] = self.buf.read_u8_err()?;
            scanline[2] = self.buf.read_u8_err()?;
            i = self.buf.read_u8_err()?;

            if scanline[1] != 2 || (scanline[2] & 128) != 0 {
                scanline[0] = 2;
                scanline[3] = i;

                self.decompress(&mut scanline[4..], self.width as i32 - 1, 0)?;
                convert_scanline(&scanline, out_scanline);
                continue;
            }

            for i in 0..4 {
                let new_scanline = &mut scanline[i..];

                let mut j = 0;

                loop {
                    if j >= self.width * 4 {
                        break;
                    }
                    let mut run = i32::from(self.buf.read_u8_err()?);

                    if run > 128 {
                        let val = self.buf.read_u8();
                        run &= 127;

                        while run > 0 {
                            run -= 1;

                            if j >= self.width * 4 {
                                break;
                            }
                            new_scanline[j] = val;
                            j += 4;
                        }
                    } else if run > 0 {
                        while run > 0 {
                            run -= 1;

                            if j >= self.width * 4 {
                                break;
                            }

                            new_scanline[j] = self.buf.read_u8();
                            j += 4;
                        }
                    }
                }
            }
            convert_scanline(&scanline, out_scanline);
        }

        Ok(())
    }

    fn decompress(
        &mut self, scanline: &mut [u8], mut width: i32, mut scanline_offset: usize
    ) -> Result<(), HdrDecodeErrors> {
        let mut shift = 0;

        while width > 0 {
            scanline[scanline_offset] = self.buf.read_u8_err()?;
            scanline[scanline_offset + 1] = self.buf.read_u8_err()?;
            scanline[scanline_offset + 2] = self.buf.read_u8_err()?;
            scanline[scanline_offset + 3] = self.buf.read_u8_err()?;

            if scanline[scanline_offset] == 1
                && scanline[scanline_offset + 1] == 1
                && scanline[scanline_offset + 2] == 1
            {
                let run = scanline[scanline_offset + 3];

                let mut i = i32::from(run) << shift;

                while width > 0 && scanline_offset > 4 && i > 0 {
                    scanline.copy_within(scanline_offset - 4..scanline_offset, 4);
                    scanline_offset += 4;
                    i -= 1;
                    width -= 4;
                }
                shift += 8;

                if shift > 16 {
                    break;
                }
            } else {
                scanline_offset += 4;
                width -= 1;
                shift = 0;
            }
        }
        Ok(())
    }

    /// Get a whole radiance line and increment the buffer
    /// cursor past that line.
    ///
    /// This will write to `write_to` appropriately
    /// resizing the buffer in case the line spans a great length
    fn get_buffer_until(
        &mut self, needle: u8, write_to: &mut Vec<u8>
    ) -> Result<usize, HdrDecodeErrors> {
        write_to.clear();
        let start = self.buf.position()?;

        while !self.buf.eof()? {
            let byte = self.buf.read_u8_err()?;
            write_to.push(byte);

            if byte == needle {
                break;
            }
        }
        let end = self.buf.position()?;

        Ok(usize::try_from(end - start).unwrap())
    }
}

fn convert_scanline(in_scanline: &[u8], out_scanline: &mut [f32]) {
    for (rgbe, out) in in_scanline
        .chunks_exact(4)
        .zip(out_scanline.chunks_exact_mut(3))
    {
        if rgbe[3] == 0 {
            out[0..3].fill(0.0);
        } else {
            // separate concerns to generate code that has better
            //  ILP
            let epxo = i32::from(rgbe[3]) - 128;

            if epxo.is_positive() {
                out[0] = convert_pos(i32::from(rgbe[0]), epxo);
                out[1] = convert_pos(i32::from(rgbe[1]), epxo);
                out[2] = convert_pos(i32::from(rgbe[2]), epxo);
            } else {
                out[0] = convert_neg(i32::from(rgbe[0]), epxo);
                out[1] = convert_neg(i32::from(rgbe[1]), epxo);
                out[2] = convert_neg(i32::from(rgbe[2]), epxo);
            }
        }
    }
}

fn ldexp_pos(x: f32, exp: u32) -> f32 {
    let pow = 1_u32.wrapping_shl(exp) as f32;
    x * pow
}
fn ldexp_neg(x: f32, exp: u32) -> f32 {
    let pow = 1_u32.wrapping_shl(exp) as f32;
    x / pow
}
/// Fast calculation of  x*(2^exp).
///
/// exp is assumed to be integer
// #[inline]
// fn ldxep(x: f32, exp: i32) -> f32 {
//     let pow = (1_i32 << (exp.abs() & 31)) as f32;
//     if exp.is_negative() {
//         // if negative 2 ^ exp is the same as 1 / (1<<exp.abs()) since
//         // 2^(-exp) is sexpressed as 1/(2^exp)
//         x / pow
//     } else {
//         // 2^exp is same as 1<<exp, but latter is way faster
//         x * pow
//     }
// }

#[inline]
fn convert_pos(val: i32, exponent: i32) -> f32 {
    let v = (val as f32) / 256.0;
    ldexp_pos(v, exponent.unsigned_abs() & 31)
}

#[inline]
fn convert_neg(val: i32, exponent: i32) -> f32 {
    let v = (val as f32) / 256.0;
    ldexp_neg(v, exponent.unsigned_abs() & 31)
}
