/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::format;
use core::fmt::{Debug, Display, Formatter};

use zune_core::bit_depth::BitType;
use zune_core::bytestream::{ZByteIoError, ZByteWriterTrait, ZWriter};
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;

/// Errors occurring during encoding
pub enum PPMEncodeErrors {
    Static(&'static str),
    TooShortInput(usize, usize),
    UnsupportedColorspace(ColorSpace),
    IoError(ZByteIoError)
}

impl Debug for PPMEncodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            PPMEncodeErrors::Static(ref errors) => {
                writeln!(f, "{errors}")
            }
            PPMEncodeErrors::TooShortInput(expected, found) => {
                writeln!(f, "Expected input of length {expected} but found {found}")
            }
            PPMEncodeErrors::UnsupportedColorspace(colorspace) => {
                writeln!(f, "Unsupported colorspace {colorspace:?} for ppm")
            }
            PPMEncodeErrors::IoError(err) => {
                writeln!(f, "I/O error: {:?}", err)
            }
        }
    }
}

impl From<ZByteIoError> for PPMEncodeErrors {
    fn from(value: ZByteIoError) -> Self {
        Self::IoError(value)
    }
}

enum PPMVersions {
    P5,
    P6,
    P7
}

impl Display for PPMVersions {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::P6 => write!(f, "P6"),
            Self::P5 => write!(f, "P5"),
            Self::P7 => write!(f, "P7")
        }
    }
}

/// A PPM encoder
///
///
///# Encoding 16 bit data
/// To encode a 16-bit image, each element needs to be
/// re-interpreted as 2 u8’s in native endian, the library will do the
/// appropriate conversions when needed
///
/// # Example
/// - Encoding 8 bit grayscale data
///```
/// use zune_core::bit_depth::BitDepth;
/// use zune_core::colorspace::ColorSpace;
/// use zune_core::options::EncoderOptions;
/// use zune_ppm::PPMEncoder;
/// use zune_ppm::PPMEncodeErrors;
///
/// fn main()-> Result<(),PPMEncodeErrors> {
///    const W:usize = 100;
///    const H:usize = 100;
///    let data:[u8;{W * H}] = std::array::from_fn(|x| (x % 256) as u8);
///    let encoder = PPMEncoder::new(&data,EncoderOptions::new(W,H,ColorSpace::Luma,BitDepth::Eight));
///    let mut write_to =vec![];
///    encoder.encode(&mut write_to)?;
///    Ok(())
/// }
/// ```
pub struct PPMEncoder<'a> {
    data:    &'a [u8],
    options: EncoderOptions
}

impl<'a> PPMEncoder<'a> {
    /// Create a new encoder which will encode the specified
    /// data whose format is contained in the options.
    ///
    /// # Note
    /// To encode 16 bit data,it still must be provided as u8 bytes
    /// in native endian.
    ///
    /// One can use [`u16::to_ne_bytes`] for this if data is in a u16 slice
    ///
    /// [`u16::to_ne_bytes`]:u16::to_ne_bytes
    pub fn new(data: &'a [u8], options: EncoderOptions) -> PPMEncoder<'a> {
        PPMEncoder { data, options }
    }

    fn encode_headers<T: ZByteWriterTrait>(
        &self, stream: &mut ZWriter<T>
    ) -> Result<(), PPMEncodeErrors> {
        let version = version_for_colorspace(self.options.colorspace()).ok_or(
            PPMEncodeErrors::UnsupportedColorspace(self.options.colorspace())
        )?;

        let width = self.options.width();
        let height = self.options.height();
        let components = self.options.colorspace().num_components();
        let max_val = self.options.depth().max_value();
        let colorspace = self.options.colorspace();

        let header = match version {
            PPMVersions::P5 | PPMVersions::P6 => {
                format!("{version}\n{width}\n{height}\n{max_val}\n")
            }
            PPMVersions::P7 => {
                let tuple_type = convert_tuple_type_to_pam(colorspace);

                format!(
                    "P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH {components}\nMAXVAL {max_val}\nTUPLTYPE {tuple_type}\n ENDHDR\n",
                )
            }
        };

        stream.write_all(header.as_bytes()).unwrap();

        Ok(())
    }
    /// Encode into a user provided buffer
    ///
    /// # Arguments
    /// - out: The output buffer to write bytes into
    ///     It is recommended that the buffer be at least [`max_out_size`](crate::encoder::max_out_size) in order
    ///     to encode successfully. In case size is not big enough , the library will bail and return an error
    ///
    /// # Returns
    /// - Ok(size): The actual number of bytes written
    /// - Err: An error in case something bad happened, contents of `out` are to be treated as invalid
    pub fn encode<T: ZByteWriterTrait>(&self, out: T) -> Result<usize, PPMEncodeErrors> {
        let found = self.data.len();
        let expected = calc_expected_size(self.options);

        if expected != found {
            return Err(PPMEncodeErrors::TooShortInput(expected, found));
        }
        let mut stream = ZWriter::new(out);
        stream.reserve(expected + 37)?; // 37 arbitrary number, chosen by divinity, guaranteed to work

        self.encode_headers(&mut stream)?;

        match self.options.depth().bit_type() {
            BitType::U8 => stream.write_all(self.data)?,
            BitType::U16 => {
                // chunk in two and write to stream
                for slice in self.data.chunks_exact(2) {
                    let byte = u16::from_ne_bytes(slice.try_into().unwrap());
                    stream.write_u16_be_err(byte)?;
                }
            }
            _ => unreachable!()
        }
        let position = stream.bytes_written();
        Ok(position)
    }
}

fn version_for_colorspace(colorspace: ColorSpace) -> Option<PPMVersions> {
    match colorspace {
        ColorSpace::Luma => Some(PPMVersions::P5),
        ColorSpace::RGB => Some(PPMVersions::P6),
        ColorSpace::RGBA | ColorSpace::LumaA => Some(PPMVersions::P7),
        _ => None
    }
}

fn convert_tuple_type_to_pam(colorspace: ColorSpace) -> &'static str {
    match colorspace {
        ColorSpace::Luma => "GRAYSCALE",
        ColorSpace::RGB => "RGB",
        ColorSpace::LumaA => "GRAYSCALE_ALPHA",
        ColorSpace::RGBA => "RGB_ALPHA",
        _ => unreachable!()
    }
}

const PPM_HEADER_SIZE: usize = 100;

/// Gives expected minimum size for which the encoder wil guarantee
/// that the given raw bytes will fit
///
#[inline]
pub fn max_out_size(options: &EncoderOptions) -> usize {
    options
        .width()
        .checked_mul(options.depth().size_of())
        .unwrap()
        .checked_mul(options.height())
        .unwrap()
        .checked_mul(options.colorspace().num_components())
        .unwrap()
        .checked_add(PPM_HEADER_SIZE)
        .unwrap()
}

fn calc_expected_size(options: EncoderOptions) -> usize {
    max_out_size(&options).checked_sub(PPM_HEADER_SIZE).unwrap()
}
