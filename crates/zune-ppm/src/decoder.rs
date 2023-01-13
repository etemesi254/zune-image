use std::fmt::{Debug, Formatter};

use log::info;
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_core::result::DecodingResult;

/// An instance of a PPM decoder
///
/// The decoder can currently decode P5 and P6 formats
pub struct PPMDecoder<'a>
{
    width:           usize,
    height:          usize,
    decoded_headers: bool,
    reader:          ZByteReader<'a>,
    colorspace:      ColorSpace,
    bit_depth:       BitDepth,
    options:         DecoderOptions
}

pub enum PPMDecodeErrors
{
    Generic(String),
    GenericStatic(&'static str),
    InvalidHeader(String),
    UnsupportedImpl(String),
    LargeDimensions(usize, usize)
}

impl Debug for PPMDecodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::Generic(val) =>
            {
                writeln!(f, "{val}")
            }
            Self::GenericStatic(val) => writeln!(f, "{val}"),
            Self::InvalidHeader(val) =>
            {
                writeln!(f, "Invalid header, reason: {val}")
            }
            Self::UnsupportedImpl(val) =>
            {
                writeln!(f, "Invalid header, reason: {val}")
            }
            Self::LargeDimensions(expected, found) =>
            {
                writeln!(
                    f,
                    "Too large dimensions, expected a value less than {expected} but found {found}"
                )
            }
        }
    }
}

impl<'a> PPMDecoder<'a>
{
    /// Create a new ppm decoder with default options
    ///
    /// # Arguments
    /// - data:PPM encoded pixels
    ///
    /// # Example
    /// ```
    /// use zune_ppm::PPMDecoder;
    /// let mut decoder = PPMDecoder::new(b"NOT VALID PPM");
    ///
    /// assert!(decoder.decode().is_err());
    /// ```
    pub fn new(data: &'a [u8]) -> PPMDecoder<'a>
    {
        PPMDecoder::new_with_options(DecoderOptions::default(), data)
    }
    /// Create a new PPM decoder with the specified options
    ///
    /// # Arguments
    /// - options: Modified options for the decoder
    /// - data: PPM encoded fata.
    /// # Example
    /// ```
    /// use zune_core::options::DecoderOptions;
    /// use zune_ppm::{PPMDecoder, ZunePPMOptions};
    /// let mut decoder = PPMDecoder::new_with_options(DecoderOptions::default(),b"NOT VALID PPM");
    ///
    /// assert!(decoder.decode().is_err());
    /// ```
    pub fn new_with_options(options: DecoderOptions, data: &'a [u8]) -> PPMDecoder<'a>
    {
        let reader = ZByteReader::new(data);

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
    /// Return Err on Error otherwise return nothing,
    pub fn read_headers(&mut self) -> Result<(), PPMDecodeErrors>
    {
        if self.reader.has(3)
        {
            let p = self.reader.get_u8();
            let version = self.reader.get_u8();

            if p != b'P'
            {
                let msg = format!("Expected P as first PPM byte but got '{}' ", p as char);

                return Err(PPMDecodeErrors::Generic(msg));
            }

            if version != b'5' && version != b'6' && version != b'7'
            {
                let msg = format!(
                    "Unsupported PPM version `{}`, supported versions are 5,6 and 7",
                    version as char
                );

                return Err(PPMDecodeErrors::Generic(msg));
            }

            if version == b'5' || version == b'6'
            {
                self.decode_p5_and_p6_header(version)?;
            }
            else if version == b'7'
            {
                self.decode_p7_header()?;
            }
        }
        else
        {
            let len = self.reader.remaining();
            let msg = format!("Expected at least 3 bytes in header but stream has {len}");

            return Err(PPMDecodeErrors::Generic(msg));
        }

        Ok(())
    }
    fn decode_p7_header(&mut self) -> Result<(), PPMDecodeErrors>
    {
        let mut seen_depth = false;
        let mut seen_width = false;
        let mut seen_height = false;
        let mut seen_max_val = false;
        let mut seen_tuple_type = false;

        'infinite: loop
        {
            if self.reader.eof()
            {
                return Err(PPMDecodeErrors::GenericStatic("No more bytes"));
            }
            skip_spaces(&mut self.reader);

            let value = get_bytes_until_whitespace(&mut self.reader);

            match value
            {
                // Notice the explicit space,
                // It's needed
                b"WIDTH " =>
                {
                    self.width = self.get_integer();

                    if self.width > self.options.max_width
                    {
                        return Err(PPMDecodeErrors::LargeDimensions(
                            self.options.max_width,
                            self.width
                        ));
                    }
                    seen_width = true;
                }
                b"HEIGHT " =>
                {
                    self.height = self.get_integer();

                    if self.height > self.options.max_height
                    {
                        return Err(PPMDecodeErrors::LargeDimensions(
                            self.options.max_height,
                            self.height
                        ));
                    }

                    seen_height = true;
                }
                b"DEPTH " =>
                {
                    let depth = self.get_integer();

                    if depth > 4
                    {
                        let msg = format!("Depth {depth} is greater than 4");
                        return Err(PPMDecodeErrors::InvalidHeader(msg));
                    }

                    seen_depth = true;
                }
                b"MAXVAL " =>
                {
                    let max_value = self.get_integer();

                    if max_value > usize::from(u16::MAX)
                    {
                        let msg = format!("MAX value {max_value} greater than 65535");

                        return Err(PPMDecodeErrors::Generic(msg));
                    }

                    if max_value > 255
                    {
                        // 16 bit
                        self.bit_depth = BitDepth::Sixteen;
                    }
                    else
                    {
                        self.bit_depth = BitDepth::Eight;
                    }
                    seen_max_val = true;
                }
                b"TUPLTYPE " =>
                {
                    let new_value = get_bytes_until_whitespace(&mut self.reader);

                    // Order matters here.
                    // we want to match RGB_ALPHA before matching RGB
                    if new_value.starts_with(b"RGB_ALPHA")
                    {
                        self.colorspace = ColorSpace::RGBA;
                    }
                    else if new_value.starts_with(b"RGB")
                    {
                        self.colorspace = ColorSpace::RGB;
                    }
                    else if new_value.starts_with(b"GRAYSCALE_ALPHA")
                    {
                        self.colorspace = ColorSpace::LumaA;
                    }
                    else if new_value.starts_with(b"GRAYSCALE")
                    {
                        self.colorspace = ColorSpace::Luma;
                    }
                    else
                    {
                        let msg = format!(
                            "Unknown/unsupported tuple type {}",
                            String::from_utf8_lossy(new_value)
                        );
                        return Err(PPMDecodeErrors::InvalidHeader(msg));
                    }
                    seen_tuple_type = true;
                }
                _ =>
                {
                    if value.starts_with(b"ENDHDR")
                    {
                        break 'infinite;
                    }
                    else
                    {
                        let msg = format!(
                            "Unknown/unsupported header declaration {}",
                            String::from_utf8_lossy(value)
                        );
                        return Err(PPMDecodeErrors::InvalidHeader(msg));
                    }
                }
            }
        }
        if !seen_max_val || !seen_tuple_type || !seen_height || !seen_width || !seen_depth
        {
            return Err(PPMDecodeErrors::GenericStatic(
                "Not all expected headers were found"
            ));
        }

        info!("Width: {}", self.width);
        info!("Height: {}", self.height);
        info!("Colorspace: {:?}", self.colorspace);
        info!("Depth: {:?}", self.bit_depth);

        Ok(())
    }
    /// Decode header types from P5 and P6 format
    fn decode_p5_and_p6_header(&mut self, version: u8) -> Result<(), PPMDecodeErrors>
    {
        let colorspace = match version
        {
            b'5' => ColorSpace::Luma,
            b'6' => ColorSpace::RGB,
            _ => unreachable!()
        };
        info!("Colorspace: {:?}", colorspace);

        self.colorspace = colorspace;

        // skip whitespace
        skip_spaces(&mut self.reader);
        // read width
        self.width = self.get_integer();

        if self.width > self.options.max_width
        {
            let msg = format!(
                "Width {} greater than max width {}",
                self.width, self.options.max_width
            );
            return Err(PPMDecodeErrors::Generic(msg));
        }
        // skip whitespace
        skip_spaces(&mut self.reader);

        self.height = self.get_integer();

        if self.height > self.options.max_height
        {
            let msg = format!(
                "Height {} greater than max height {}",
                self.width, self.options.max_height
            );
            return Err(PPMDecodeErrors::Generic(msg));
        }

        info!("Width: {}, height: {}", self.width, self.height);

        skip_spaces(&mut self.reader);
        // read max value
        let max_value = self.get_integer();
        // skip ascii space
        skip_spaces(&mut self.reader);

        if max_value > usize::from(u16::MAX)
        {
            let msg = format!("MAX value {max_value} greater than 65535");

            return Err(PPMDecodeErrors::Generic(msg));
        }

        if max_value > 255
        {
            // 16 bit
            self.bit_depth = BitDepth::Sixteen;
        }

        info!("Bit Depth: {:?}", self.bit_depth);
        self.decoded_headers = true;

        Ok(())
    }
    fn get_integer(&mut self) -> usize
    {
        let mut value = 0_usize;

        while !self.reader.eof()
        {
            let byte = self.reader.get_u8();

            if byte.is_ascii_digit()
            {
                // if it overflows, we have bigger problems.
                value = value
                    .wrapping_mul(10_usize)
                    .wrapping_add(usize::from(byte - b'0'))
            }
            else
            {
                // rewind to the previous byte
                self.reader.rewind(1);
                break;
            }
        }
        value
    }

    /// Return the image bit depth or none if headers
    /// are not decoded
    pub const fn get_bit_depth(&self) -> Option<BitDepth>
    {
        if self.decoded_headers
        {
            Some(self.bit_depth)
        }
        else
        {
            None
        }
    }
    /// Return the image colorspace or none if
    /// headers aren't decoded
    pub const fn get_colorspace(&self) -> Option<ColorSpace>
    {
        if self.decoded_headers
        {
            Some(self.colorspace)
        }
        else
        {
            None
        }
    }
    /// Return image dimensions or none if image isn't decoded
    pub const fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        if self.decoded_headers
        {
            Some((self.width, self.height))
        }
        else
        {
            None
        }
    }
    /// Decode a ppm encoded file and return the row bytes from it
    ///
    /// DecodingResult is an enum that can have either Vec<u8> or Vec<u16>,
    /// and that depends on image bit depth.
    pub fn decode(&mut self) -> Result<DecodingResult, PPMDecodeErrors>
    {
        self.read_headers()?;
        // okay check if the stream is large enough for the bit depth
        let size =
            self.width * self.height * self.colorspace.num_components() * self.bit_depth.size_of();

        let remaining = self.reader.remaining();

        if size != remaining
        {
            let msg = format!("Expected {size} number of bytes but found {remaining}");

            return Err(PPMDecodeErrors::Generic(msg));
        }
        return match self.bit_depth.bit_type()
        {
            BitType::Eight =>
            {
                let mut data = vec![0; size];
                // get the bytes
                data.copy_from_slice(self.reader.get_as_ref(size).unwrap());

                Ok(DecodingResult::U8(data))
            }
            BitType::Sixteen =>
            {
                // size is divided by 2 since sizeof added 2 for u16
                // and when channel stores u16 it uses double the size
                // as that of u8
                let mut data = vec![0_u16; size / 2];

                for datum in data.iter_mut()
                {
                    // 16 bit ppm happens to be written in big-endian
                    // i.e that's what is supported by netbpm
                    // So we also do emulate that.
                    *datum = self.reader.get_u16_be();
                }

                Ok(DecodingResult::U16(data))
            }
        };
    }
}

/// Skip all whitespace characters and comments
/// until one hits a character that isn't a space or
/// we reach eof
fn skip_spaces(byte_stream: &mut ZByteReader)
{
    while !byte_stream.eof()
    {
        let mut byte = byte_stream.get_u8();

        if byte == b'#'
        {
            // comment
            // skip the whole comment
            while byte != b'\n' && !byte_stream.eof()
            {
                byte = byte_stream.get_u8();
            }
        }
        else if !byte.is_ascii_whitespace()
        {
            // go back one step, we hit something that is not a space
            byte_stream.rewind(1);
            break;
        }
    }
}

/// Return a reference to all bytes preceding a whitespace.
///
/// # Note
/// This skips all whitespaces after it finds one. That is the desired implementation
///
/// # Panics
/// If end < start
fn get_bytes_until_whitespace<'a>(z: &'a mut ZByteReader) -> &'a [u8]
{
    let start = z.get_position();

    while !z.eof()
    {
        let byte = z.get_u8();
        if byte.is_ascii_whitespace()
        {
            // skip any proceeding whitespace
            skip_spaces(z);
            break;
        }
    }
    let end = z.get_position();
    // rewind back to where we currently were
    z.rewind(end - start);
    // then take that as a reference
    let stream = z.peek_at(0, end - start).unwrap();
    // then bump up position to indicate we read those bytes
    z.skip(end - start);
    stream
}
