/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};
use core::fmt::{Debug, Formatter};

use zune_core::bit_depth::{BitDepth, BitType, ByteEndian};
use zune_core::bytestream::{ZByteIoError, ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::log::trace;
use zune_core::options::DecoderOptions;
use zune_core::result::DecodingResult;

/// An instance of a PPM decoder
///
/// The decoder can currently decode P5 and P6 formats
pub struct PPMDecoder<T>
where
    T: ZByteReaderTrait
{
    width:           usize,
    height:          usize,
    decoded_headers: bool,
    reader:          ZReader<T>,
    colorspace:      ColorSpace,
    bit_depth:       BitDepth,
    options:         DecoderOptions
}

/// Decoding errors that may occur
pub enum PPMDecodeErrors {
    Generic(String),
    GenericStatic(&'static str),
    /// There is a problem with the header
    /// of a ppm file.
    InvalidHeader(String),
    /// The PPM format is not supported
    UnsupportedImpl(String),
    /// The PPM file in question has larger dimensions(width,height)
    /// than the accepted one
    LargeDimensions(usize, usize),
    IoErrors(ZByteIoError)
}
impl From<ZByteIoError> for PPMDecodeErrors {
    fn from(value: ZByteIoError) -> Self {
        PPMDecodeErrors::IoErrors(value)
    }
}

impl Debug for PPMDecodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Generic(val) => {
                writeln!(f, "{val}")
            }
            Self::GenericStatic(val) => writeln!(f, "{val}"),
            Self::InvalidHeader(val) => {
                writeln!(f, "Invalid header, reason: {val}")
            }
            Self::UnsupportedImpl(val) => {
                writeln!(f, "Invalid header, reason: {val}")
            }
            Self::LargeDimensions(expected, found) => {
                writeln!(
                    f,
                    "Too large dimensions, expected a value less than {expected} but found {found}"
                )
            }
            Self::IoErrors(err) => {
                writeln!(f, "{:?}", err)
            }
        }
    }
}

impl<T> PPMDecoder<T>
where
    T: ZByteReaderTrait
{
    /// Create a new ppm decoder with default options
    ///
    /// # Arguments
    /// - data:PPM encoded pixels
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZCursor;
    /// use zune_ppm::PPMDecoder;
    /// let mut decoder = PPMDecoder::new(ZCursor::new(b"NOT VALID PPM"));
    ///
    /// assert!(decoder.decode().is_err());
    /// ```
    pub fn new(data: T) -> PPMDecoder<T> {
        PPMDecoder::new_with_options(data, DecoderOptions::default())
    }
    /// Create a new PPM decoder with the specified options
    ///
    /// # Arguments
    /// - options: Modified options for the decoder
    /// - data: PPM encoded fata.
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZCursor;
    /// use zune_core::options::DecoderOptions;
    /// use zune_ppm::PPMDecoder;
    /// let mut decoder = PPMDecoder::new_with_options(ZCursor::new(b"NOT VALID PPM"),DecoderOptions::default());
    ///
    /// assert!(decoder.decode().is_err());
    /// ```
    pub fn new_with_options(data: T, options: DecoderOptions) -> PPMDecoder<T> {
        let reader = ZReader::new(data);

        PPMDecoder {
            width: 0,
            height: 0,
            decoded_headers: false,
            reader,
            colorspace: ColorSpace::Unknown,
            bit_depth: BitDepth::Eight,
            options
        }
    }
    /// Read PPM headers and store them in internal state
    ///
    /// After this, information about the image can be accessed by other
    /// accessors like [`get_dimensions`] to get image dimensions
    ///
    /// # Returns
    /// - `()` : On successful decode, items can be accessed by accessors
    ///
    /// - `Err(PPMDecodeErrors)`: This will return an `InvalidHeader`  enum, the string
    /// will more information about what went wrong
    ///
    /// [`get_dimensions`]:Self::dimensions
    pub fn decode_headers(&mut self) -> Result<(), PPMDecodeErrors> {
        let p = self.reader.read_u8_err()?;
        let version = self.reader.read_u8_err()?;

        if p != b'P' {
            let msg = format!("Expected P as first PPM byte but got '{}' ", p as char);

            return Err(PPMDecodeErrors::InvalidHeader(msg));
        }

        if version == b'5' || version == b'6' {
            self.decode_p5_and_p6_header(version)?;
        } else if version == b'7' {
            self.decode_p7_header()?;
        } else if version == b'f' {
            self.decode_pf_header(ColorSpace::Luma)?;
        } else if version == b'F' {
            self.decode_pf_header(ColorSpace::RGB)?;
        } else {
            let msg = format!(
                "Unsupported PPM version `{}`, supported versions are 5,6 and 7",
                version as char
            );

            return Err(PPMDecodeErrors::InvalidHeader(msg));
        }

        Ok(())
    }
    fn decode_pf_header(&mut self, colorspace: ColorSpace) -> Result<(), PPMDecodeErrors> {
        self.colorspace = colorspace;
        // read width and height
        // skip whitespace
        skip_spaces(&mut self.reader)?;
        // read width
        self.width = self.get_integer()?;

        if self.width > self.options.max_width() {
            let msg = format!(
                "Width {} greater than max width {}",
                self.width,
                self.options.max_width()
            );
            return Err(PPMDecodeErrors::Generic(msg));
        }
        // skip whitespace
        skip_spaces(&mut self.reader)?;

        self.height = self.get_integer()?;

        if self.height > self.options.max_height() {
            let msg = format!(
                "Height {} greater than max height {}",
                self.width,
                self.options.max_height()
            );
            return Err(PPMDecodeErrors::Generic(msg));
        }

        trace!("Width: {}, height: {}", self.width, self.height);

        skip_spaces(&mut self.reader)?;

        let mut byte_header = Vec::with_capacity(20);

        let value_size = get_bytes_until_whitespace(&mut self.reader, &mut byte_header)?;
        let value = &byte_header[..value_size];

        // get the magnitude byte
        let int_bytes = match core::str::from_utf8(value) {
            Ok(valid_str) => match valid_str.trim().parse::<f32>() {
                Ok(number) => number,
                Err(_) => {
                    return Err(PPMDecodeErrors::Generic(format!(
                        "Invalid number {valid_str:?}"
                    )))
                }
            },
            Err(_) => return Err(PPMDecodeErrors::GenericStatic("Invalid string"))
        };
        // " is a number used to indicate the byte order within the file.
        // A positive number (e.g. "1.0") indicates big-endian
        //
        // If the number is negative (e.g. "-1.0") this indicates little-endian, with the least significant byte first.
        if int_bytes < 0.0 {
            self.options = self.options.set_byte_endian(ByteEndian::LE);
        } else {
            self.options = self.options.set_byte_endian(ByteEndian::BE);
        }
        self.decoded_headers = true;
        self.bit_depth = BitDepth::Float32;

        Ok(())
    }
    /// Decode header types from P7 format
    fn decode_p7_header(&mut self) -> Result<(), PPMDecodeErrors> {
        let mut seen_depth = false;
        let mut seen_width = false;
        let mut seen_height = false;
        let mut seen_max_val = false;
        let mut seen_tuple_type = false;

        let mut byte_header = Vec::with_capacity(20);

        'infinite: loop {
            if self.reader.eof()? {
                return Err(PPMDecodeErrors::InvalidHeader("No more bytes".to_string()));
            }
            skip_spaces(&mut self.reader)?;

            let value_size = get_bytes_until_whitespace(&mut self.reader, &mut byte_header)?;
            let value = &byte_header[..value_size];

            match value {
                // Notice the explicit space,
                // It's needed
                b"WIDTH " => {
                    self.width = self.get_integer()?;

                    if self.width > self.options.max_width() {
                        return Err(PPMDecodeErrors::LargeDimensions(
                            self.options.max_width(),
                            self.width
                        ));
                    }
                    seen_width = true;
                }
                b"HEIGHT " => {
                    self.height = self.get_integer()?;

                    if self.height > self.options.max_height() {
                        return Err(PPMDecodeErrors::LargeDimensions(
                            self.options.max_height(),
                            self.height
                        ));
                    }

                    seen_height = true;
                }
                b"DEPTH " => {
                    let depth = self.get_integer()?;

                    if depth > 4 {
                        let msg = format!("Depth {depth} is greater than 4");
                        return Err(PPMDecodeErrors::InvalidHeader(msg));
                    }

                    seen_depth = true;
                }
                b"MAXVAL " => {
                    let max_value = self.get_integer()?;

                    if max_value > usize::from(u16::MAX) {
                        let msg = format!("MAX value {max_value} greater than 65535");

                        return Err(PPMDecodeErrors::Generic(msg));
                    }

                    if max_value > 255 {
                        // 16 bit
                        self.bit_depth = BitDepth::Sixteen;
                    } else {
                        self.bit_depth = BitDepth::Eight;
                    }
                    seen_max_val = true;
                }
                b"TUPLTYPE " => {
                    let value_size =
                        get_bytes_until_whitespace(&mut self.reader, &mut byte_header)?;
                    let new_value = &byte_header[..value_size];

                    // Order matters here.
                    // we want to match RGB_ALPHA before matching RGB
                    if new_value.starts_with(b"RGB_ALPHA") {
                        self.colorspace = ColorSpace::RGBA;
                    } else if new_value.starts_with(b"RGB") {
                        self.colorspace = ColorSpace::RGB;
                    } else if new_value.starts_with(b"GRAYSCALE_ALPHA") {
                        self.colorspace = ColorSpace::LumaA;
                    } else if new_value.starts_with(b"GRAYSCALE") {
                        self.colorspace = ColorSpace::Luma;
                    } else {
                        let msg = format!(
                            "Unknown/unsupported tuple type {}",
                            String::from_utf8_lossy(new_value)
                        );
                        return Err(PPMDecodeErrors::InvalidHeader(msg));
                    }
                    seen_tuple_type = true;
                }
                _ => {
                    if value.starts_with(b"ENDHDR") {
                        break 'infinite;
                    } else {
                        let msg = format!(
                            "Unknown/unsupported header declaration {}",
                            String::from_utf8_lossy(value)
                        );
                        return Err(PPMDecodeErrors::InvalidHeader(msg));
                    }
                }
            }
        }
        if !seen_max_val || !seen_tuple_type || !seen_height || !seen_width || !seen_depth {
            return Err(PPMDecodeErrors::InvalidHeader(
                "Not all expected headers were found".to_string()
            ));
        }

        self.decoded_headers = true;

        trace!("Width: {}", self.width);
        trace!("Height: {}", self.height);
        trace!("Colorspace: {:?}", self.colorspace);
        trace!("Depth: {:?}", self.bit_depth);

        Ok(())
    }
    /// Decode header types from P5 and P6 format
    fn decode_p5_and_p6_header(&mut self, version: u8) -> Result<(), PPMDecodeErrors> {
        let colorspace = match version {
            b'5' => ColorSpace::Luma,
            b'6' => ColorSpace::RGB,
            _ => unreachable!()
        };
        trace!("Colorspace: {:?}", colorspace);

        self.colorspace = colorspace;

        // skip whitespace
        skip_spaces(&mut self.reader)?;
        // read width
        self.width = self.get_integer()?;

        if self.width > self.options.max_width() {
            let msg = format!(
                "Width {} greater than max width {}",
                self.width,
                self.options.max_width()
            );
            return Err(PPMDecodeErrors::Generic(msg));
        }
        // skip whitespace
        skip_spaces(&mut self.reader)?;

        self.height = self.get_integer()?;

        if self.height > self.options.max_height() {
            let msg = format!(
                "Height {} greater than max height {}",
                self.width,
                self.options.max_height()
            );
            return Err(PPMDecodeErrors::Generic(msg));
        }

        trace!("Width: {}, height: {}", self.width, self.height);

        skip_spaces(&mut self.reader)?;
        // read max value
        let max_value = self.get_integer()?;
        // skip ascii space
        skip_spaces(&mut self.reader)?;

        if max_value > usize::from(u16::MAX) {
            let msg = format!("MAX value {max_value} greater than 65535");

            return Err(PPMDecodeErrors::Generic(msg));
        }

        if max_value > 255 {
            // 16 bit
            self.bit_depth = BitDepth::Sixteen;
        }

        trace!("Bit Depth: {:?}", self.bit_depth);
        self.decoded_headers = true;

        Ok(())
    }

    fn get_integer(&mut self) -> Result<usize, PPMDecodeErrors> {
        let mut value = 0_usize;

        while !self.reader.eof()? {
            let byte = self.reader.read_u8();

            if byte.is_ascii_digit() {
                // if it overflows, we have bigger problems.
                value = value
                    .wrapping_mul(10_usize)
                    .wrapping_add(usize::from(byte - b'0'))
            } else {
                // rewind to the previous byte
                self.reader.rewind(1)?;
                break;
            }
        }
        Ok(value)
    }

    /// Return the image bit depth or none if headers
    /// are not decoded.
    ///
    /// # Returns
    /// - `Some(BitDepth)`: The image bit depth, can be Eight or Sixteen, or F32 for (.pfm files)
    /// - `None`: Indicates the header wasn't decoded or there was an unhandled error
    /// in parsing
    ///
    pub const fn bit_depth(&self) -> Option<BitDepth> {
        if self.decoded_headers {
            Some(self.bit_depth)
        } else {
            None
        }
    }
    /// Return the image colorspace or none if
    /// headers aren't decoded
    ///
    /// # Returns
    /// - `Some(ColorSpace)`: The colorspace of the input image
    /// - None: Indicates headers weren't decoded or an unhandled error occurred
    /// during header decoding
    pub const fn colorspace(&self) -> Option<ColorSpace> {
        if self.decoded_headers {
            Some(self.colorspace)
        } else {
            None
        }
    }
    /// Return image dimensions or none if image isn't decoded
    ///
    /// # Returns
    /// - `Some(width,height)`: The image width and height as a usize
    /// -  None: Indicates the image headers weren't decoded or an error occurred
    ///
    ///  # Example
    /// ```
    /// use std::io::Cursor;
    /// use zune_core::bit_depth::BitDepth;
    /// use zune_ppm::PPMDecoder;
    /// // a simple ppm header
    /// let data = b"P6 34 32 255";
    /// let mut decoder = PPMDecoder::new(Cursor::new(data));
    ///
    /// decoder.decode_headers().unwrap();
    ///
    /// assert_eq!(decoder.bit_depth(),Some(BitDepth::Eight));
    /// assert_eq!(decoder.dimensions(),Some((34,32)))
    /// ```
    pub const fn dimensions(&self) -> Option<(usize, usize)> {
        if self.decoded_headers {
            Some((self.width, self.height))
        } else {
            None
        }
    }
    /// Decode a ppm encoded file and return the row bytes from it
    ///
    /// DecodingResult is an enum that can have either `Vec<u8>` or `Vec<u16>`,
    /// and that depends on image bit depth.
    ///
    /// # Returns
    /// - `Ok(DecodingResult)`: This is a simple enum that can hold either
    /// eight or 16 bits ([`u8`] or [`u16`]) singe ppm images can either be 8 bit or 16 bit.
    ///    It can also return `DecodingResult::F32` in case of decoding PFM  images
    ///
    ///  -  Err(PPMDecodeErrors)`: There was a problem
    /// # Example
    /// ```
    /// use zune_ppm::PPMDecoder;
    /// use zune_core::bit_depth::BitDepth;
    /// use zune_core::bytestream::ZCursor;
    /// // a 1 by 1 grayscale 16 bit ppm
    /// let data = b"P5 1 1 65535 23";
    ///
    /// let mut decoder = PPMDecoder::new(ZCursor::new(data));
    ///
    /// decoder.decode_headers().unwrap();
    ///
    /// assert_eq!(decoder.bit_depth(),Some(BitDepth::Sixteen));
    /// assert_eq!(decoder.dimensions(),Some((1,1)));
    /// let bytes = decoder.decode().unwrap();
    ///
    /// assert_eq!(&bytes.u16().unwrap(),&[12851]); // 23 in ascii is 12851
    ///
    /// ```
    pub fn decode(&mut self) -> Result<DecodingResult, PPMDecodeErrors> {
        // decode headers only if no previous call was made.
        if !self.decoded_headers {
            self.decode_headers()?;
        }

        if self.width == 0 || self.height == 0 {
            return Err(PPMDecodeErrors::GenericStatic(
                "Zero dimensions not allowed"
            ));
        }
        // okay check if the stream is large enough for the bit depth
        let size =
            self.width * self.height * self.colorspace.num_components() * self.bit_depth.size_of();

        return match self.bit_depth.bit_type() {
            BitType::U8 => {
                let mut data = vec![0; size];
                // get the bytes
                //data.copy_from_slice(self.reader.get(size).unwrap());
                self.reader.read_exact_bytes(&mut data)?;

                Ok(DecodingResult::U8(data))
            }
            BitType::U16 => {
                // size is divided by 2 since sizeof added 2 for u16
                // and when channel stores u16 it uses double the size
                // as that of u8

                // Get bytes from heaven.
                // This saves us the memset part of vec![0;size/2]; by
                // borrowing uninitialized memory from the heap
                //let remaining = self.reader.remaining_bytes();
                let mut data = vec![0; size / 2];

                for datum in &mut data {
                    *datum = self.reader.get_u16_be_err()?;
                }

                Ok(DecodingResult::U16(data))
            }
            BitType::F32 => {
                // match endianness
                // specified by the decoder options
                let mut result = if self.options.byte_endian() == ByteEndian::BE {
                    let mut output =
                        vec![0.0f32; self.width * self.height * self.colorspace.num_components()];

                    for out in &mut output {
                        // TODO: Should it be ne or be?
                        *out = f32::from_bits(self.reader.get_u32_be_err()?);
                    }
                    output
                } else if self.options.byte_endian() == ByteEndian::LE {
                    let mut output =
                        vec![0.0f32; self.width * self.height * self.colorspace.num_components()];

                    for out in &mut output {
                        // TODO: Should it be ne or be?
                        *out = f32::from_bits(self.reader.get_u32_le_err()?);
                    }
                    output
                } else {
                    unreachable!()
                };

                // pfm uses bottom-top orientation, so let's fix it
                let length = result.len() / 2;

                let (in_img_top, in_img_bottom) = result.split_at_mut(length);

                let single_stride = self.width * self.colorspace.num_components();
                let mut stride = vec![0.; single_stride];

                for (in_dim, out_dim) in in_img_top
                    .chunks_exact_mut(single_stride)
                    .rev()
                    .zip(in_img_bottom.chunks_exact_mut(single_stride))
                {
                    // copy in_top to temp vec
                    stride.copy_from_slice(in_dim);
                    // copy bottom to top
                    in_dim.copy_from_slice(out_dim);
                    // copy temp to bottom
                    out_dim.copy_from_slice(&stride);
                }

                Ok(DecodingResult::F32(result))
            }
            _ => unreachable!()
        };
    }
}

/// Skip all whitespace characters and comments
/// until one hits a character that isn't a space or
/// we reach eof
fn skip_spaces<T>(byte_stream: &mut ZReader<T>) -> Result<(), PPMDecodeErrors>
where
    T: ZByteReaderTrait
{
    while !byte_stream.eof()? {
        let mut byte = byte_stream.read_u8();

        if byte == b'#' {
            // comment
            // skip the whole comment
            while byte != b'\n' && !byte_stream.eof()? {
                byte = byte_stream.read_u8();
            }
        } else if !byte.is_ascii_whitespace() {
            // go back one step, we hit something that is not a space
            byte_stream.rewind(1)?;
            break;
        }
    }
    Ok(())
}

/// Return a reference to all bytes preceding a whitespace.
///
/// # Note
/// This skips all whitespaces after it finds one. That is the desired implementation
///
/// # Panics
/// If end < start
fn get_bytes_until_whitespace<T: ZByteReaderTrait>(
    z: &mut ZReader<T>, write_to: &mut Vec<u8>
) -> Result<usize, PPMDecodeErrors> {
    let start = z.position()?;
    let mut end = start;
    // clear out buffer for the next iteration
    write_to.clear();

    while !z.eof()? {
        let byte = z.read_u8();
        write_to.push(byte);

        if byte.is_ascii_whitespace() {
            // mark where the text ends
            end = z.position()?;
            // skip any proceeding whitespace
            skip_spaces(z)?;
            break;
        }
        // push the byte read
    }
    // z.skip(end - start);
    Ok((end - start) as usize)
}
