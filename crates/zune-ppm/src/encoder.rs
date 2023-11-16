/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::vec::Vec;
use alloc::{format, vec};
use core::fmt::{Debug, Display, Formatter};

use zune_core::bit_depth::BitType;
use zune_core::bytestream::ZByteWriter;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;

/// Errors occurring during encoding
pub enum PPMEncodeErrors {
    Static(&'static str),
    TooShortInput(usize, usize),
    UnsupportedColorspace(ColorSpace)
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
        }
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
///    encoder.encode()?;
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

    fn encode_headers(&self, stream: &mut ZByteWriter) -> Result<(), PPMEncodeErrors> {
        let version = version_for_colorspace(self.options.get_colorspace()).ok_or(
            PPMEncodeErrors::UnsupportedColorspace(self.options.get_colorspace())
        )?;

        let width = self.options.get_width();
        let height = self.options.get_height();
        let components = self.options.get_colorspace().num_components();
        let max_val = self.options.get_depth().max_value();
        let colorspace = self.options.get_colorspace();

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
    pub fn encode_into(&self, out: &mut [u8]) -> Result<usize, PPMEncodeErrors> {
        let expected = calc_expected_size(self.options);
        let found = self.data.len();

        if expected != found {
            return Err(PPMEncodeErrors::TooShortInput(expected, found));
        }
        let mut stream = ZByteWriter::new(out);

        self.encode_headers(&mut stream)?;

        match self.options.get_depth().bit_type() {
            BitType::U8 => stream
                .write_all(self.data)
                .map_err(|x| PPMEncodeErrors::Static(x))?,
            BitType::U16 => {
                if !stream.has(self.data.len()) {
                    return Err(PPMEncodeErrors::Static("The data will not fit into buffer"));
                }
                // chunk in two and write to stream
                for slice in self.data.chunks_exact(2) {
                    let byte = u16::from_ne_bytes(slice.try_into().unwrap());
                    stream.write_u16_be(byte)
                }
            }
            _ => unreachable!()
        }
        assert!(!stream.eof());
        let position = stream.position();
        Ok(position)
    }
    /// Encode an image returning the pixels as a `Vec<u8>` or an error
    /// in case something happened
    ///
    /// # Returns
    /// - Ok(vec): The actual number of bytes written
    /// - Err : An error that occurred during encoding in case it happens
    pub fn encode(&self) -> Result<Vec<u8>, PPMEncodeErrors> {
        let out_size = max_out_size(&self.options);

        let mut out = vec![0; out_size];

        let position = self.encode_into(&mut out)?;
        // truncate to how many bytes we wrote
        out.truncate(position);

        Ok(out)
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

/// Gives expected minimum size for which the encoder wil guarrantee
/// that the given raw bytes will fit
///
/// This can be used with [`encode_into`](crate::encoder::PPMEncoder::encode_into) to
/// properly allocate an input buffer to be used for encoding
#[inline]
pub fn max_out_size(options: &EncoderOptions) -> usize {
    options
        .get_width()
        .checked_mul(options.get_depth().size_of())
        .unwrap()
        .checked_mul(options.get_height())
        .unwrap()
        .checked_mul(options.get_colorspace().num_components())
        .unwrap()
        .checked_add(PPM_HEADER_SIZE)
        .unwrap()
}

fn calc_expected_size(options: EncoderOptions) -> usize {
    max_out_size(&options).checked_sub(PPM_HEADER_SIZE).unwrap()
}
