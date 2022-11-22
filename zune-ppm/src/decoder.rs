use std::fmt::{Debug, Formatter};

use log::info;
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;

/// PPM options one can use to influence PPM decoding
#[derive(Copy, Clone)]
pub struct ZunePPMOptions
{
    max_width:  usize,
    max_height: usize
}

impl ZunePPMOptions
{
    pub fn set_max_width(&mut self, width: usize)
    {
        self.max_width = width;
    }
    pub fn set_max_height(&mut self, height: usize)
    {
        self.max_height = height;
    }
    pub const fn get_max_width(&self) -> usize
    {
        self.max_width
    }
    pub const fn get_max_height(&self) -> usize
    {
        self.max_height
    }
}

pub enum DecodingResult
{
    U8(Vec<u8>),
    U16(Vec<u16>)
}

impl Default for ZunePPMOptions
{
    fn default() -> Self
    {
        Self {
            max_height: 1 << 17,
            max_width:  1 << 17
        }
    }
}

pub struct PPMDecoder<'a>
{
    width:           usize,
    height:          usize,
    decoded_headers: bool,
    reader:          ZByteReader<'a>,
    colorspace:      ColorSpace,
    bit_depth:       BitDepth,
    options:         ZunePPMOptions
}

pub enum PPMDecodeErrors
{
    Generic(String),
    GenericStatic(&'static str)
}

impl Debug for PPMDecodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::Generic(val) =>
            {
                writeln!(f, "{}", val)
            }
            Self::GenericStatic(val) => writeln!(f, "{}", val)
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
        PPMDecoder::new_with_options(ZunePPMOptions::default(), data)
    }
    /// Create a new PPM decoder with the specified options
    ///
    /// # Arguments
    /// - options: Modified options for the decoder
    /// - data: PPM encoded fata.
    /// # Example
    /// ```
    /// use zune_ppm::{PPMDecoder, ZunePPMOptions};
    /// let mut decoder = PPMDecoder::new_with_options(ZunePPMOptions::default(),b"NOT VALID PPM");
    ///
    /// assert!(decoder.decode().is_err());
    /// ```
    pub fn new_with_options(options: ZunePPMOptions, data: &'a [u8]) -> PPMDecoder<'a>
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

            if version != b'5' && version != b'6'
            {
                let msg = format!(
                    "Unsupported PPM version `{}`, supported versions are 5 and 6",
                    version as char
                );

                return Err(PPMDecodeErrors::Generic(msg));
            }

            let colorspace = match version
            {
                b'5' => ColorSpace::Luma,
                b'6' => ColorSpace::RGB,
                _ => unreachable!()
            };
            info!("Colorspace: {:?}", colorspace);

            self.colorspace = colorspace;

            // skip whitespace
            self.reader.skip_until_false(|x| x.is_ascii_whitespace());
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
            self.reader.skip_until_false(|x| x.is_ascii_whitespace());

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

            self.reader.skip_until_false(|x| x.is_ascii_whitespace());
            // read max value
            let max_value = self.get_integer();
            // skip ascii space
            self.reader.skip_until_false(|x| x.is_ascii_whitespace());

            if max_value > usize::from(u16::MAX)
            {
                let msg = format!("MAX value {} greater than 65535", max_value);

                return Err(PPMDecodeErrors::Generic(msg));
            }

            if max_value > 255
            {
                // 16 bit
                self.bit_depth = BitDepth::Sixteen;
            }

            info!("Bit Depth: {:?}", self.bit_depth);
            self.decoded_headers = true;
        }
        else
        {
            let len = self.reader.remaining();
            let msg = format!("Expected at least 3 bytes in header but stream has {}", len);

            return Err(PPMDecodeErrors::Generic(msg));
        }

        Ok(())
    }
    fn get_integer(&mut self) -> usize
    {
        let mut value = 0;

        while !self.reader.eof()
        {
            let byte = self.reader.get_u8();

            if byte.is_ascii_digit()
            {
                value = value * 10 + usize::from(byte - b'0')
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
            let msg = format!("Expected {} number of bytes but found {}", size, remaining);

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
                // size is divided by 2 since sizeof added 2
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

#[test]
fn hello_test()
{
    use std::fs::read;
    let data = read("/home/caleb/tt.ppm").unwrap();
    let mut decoder = PPMDecoder::new(&data);
    decoder.decode().unwrap();
}
