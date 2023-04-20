use core::fmt::{Debug, Formatter};
use std::num::ParseIntError;

pub enum HdrDecodeErrors
{
    InvalidMagicBytes,
    ParseError(ParseIntError),
    UnsupportedOrientation(String, String),
    TooLargeDimensions(&'static str, usize, usize),
    Generic(&'static str),
    TooSmallOutputArray(usize, usize)
}

impl Debug for HdrDecodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result
    {
        match self
        {
            HdrDecodeErrors::InvalidMagicBytes =>
            {
                writeln!(
                    f,
                    "Invalid magic bytes, file does not start with #?RADIANCE or #?RGBE"
                )
            }
            HdrDecodeErrors::ParseError(err) =>
            {
                writeln!(f, "Could not parse integer {:?}", err)
            }
            HdrDecodeErrors::UnsupportedOrientation(x, y) =>
            {
                writeln!(f, "Unsupported image orientation of {x} {y}")
            }
            HdrDecodeErrors::TooLargeDimensions(dimension, expected, found) =>
            {
                writeln!(
                    f,
                    "Too large dimensions for {dimension} , {found} exceeds {expected}"
                )
            }
            HdrDecodeErrors::Generic(error) =>
            {
                writeln!(f, "{error}")
            }
            HdrDecodeErrors::TooSmallOutputArray(expected, found) =>
            {
                writeln!(f, "Too small of an output array, expected array of at least length {} but found {}", expected, found)
            }
        }
    }
}

impl From<ParseIntError> for HdrDecodeErrors
{
    fn from(value: ParseIntError) -> Self
    {
        HdrDecodeErrors::ParseError(value)
    }
}
