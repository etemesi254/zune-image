/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::string::String;
use core::convert::From;
use core::fmt::{Debug, Display, Formatter};
use core::num::ParseIntError;

use zune_core::bytestream::ZByteIoError;
use zune_core::colorspace::ColorSpace;

/// HDR decoding errors
pub enum HdrDecodeErrors {
    /// Magic bytes do not start with `?#RADIANCE` or `?#RGBE`
    InvalidMagicBytes,
    /// The decoder could not convert string to int
    ParseError(ParseIntError),
    /// The image contains an unsupported orientation
    UnsupportedOrientation(String, String),
    /// Too large dimensions for a given dimension
    TooLargeDimensions(&'static str, usize, usize),
    /// Generic message
    Generic(&'static str),
    /// The output array is too small to contain the whole
    /// image
    TooSmallOutputArray(usize, usize),
    IoErrors(ZByteIoError)
}

impl Debug for HdrDecodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            HdrDecodeErrors::InvalidMagicBytes => {
                writeln!(
                    f,
                    "Invalid magic bytes, file does not start with #?RADIANCE or #?RGBE"
                )
            }
            HdrDecodeErrors::ParseError(err) => {
                writeln!(f, "Could not parse integer {:?}", err)
            }
            HdrDecodeErrors::UnsupportedOrientation(x, y) => {
                writeln!(f, "Unsupported image orientation of {x} {y}")
            }
            HdrDecodeErrors::TooLargeDimensions(dimension, expected, found) => {
                writeln!(
                    f,
                    "Too large dimensions for {dimension} , {found} exceeds {expected}"
                )
            }
            HdrDecodeErrors::Generic(error) => {
                writeln!(f, "{error}")
            }
            HdrDecodeErrors::TooSmallOutputArray(expected, found) => {
                writeln!(f, "Too small of an output array, expected array of at least length {} but found {}", expected, found)
            }
            HdrDecodeErrors::IoErrors(err) => {
                writeln!(f, "{:?}", err)
            }
        }
    }
}

impl From<ParseIntError> for HdrDecodeErrors {
    fn from(value: ParseIntError) -> Self {
        HdrDecodeErrors::ParseError(value)
    }
}

impl From<ZByteIoError> for HdrDecodeErrors {
    fn from(value: ZByteIoError) -> Self {
        HdrDecodeErrors::IoErrors(value)
    }
}
impl Display for HdrDecodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "{:?}", self)
    }
}
impl std::error::Error for HdrDecodeErrors {}

impl Display for HdrEncodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "{:?}", self)
    }
}

impl std::error::Error for HdrEncodeErrors {}

/// HDR encoding errrors
pub enum HdrEncodeErrors {
    /// The colorspace provided by user is not supported by HDR
    UnsupportedColorspace(ColorSpace),
    /// The input size was expected to be of a certain size but isn't
    WrongInputSize(usize, usize),
    /// Generic message
    Static(&'static str),
    IoErrors(ZByteIoError)
}

impl Debug for HdrEncodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            HdrEncodeErrors::UnsupportedColorspace(color) => {
                writeln!(f, "Unsupported colorspace {color:?} for Radiance, Radiance only works with RGB f32 data")
            }
            HdrEncodeErrors::WrongInputSize(expected, found) => {
                writeln!(f, "Input array length {found} doesn't match {expected}")
            }
            HdrEncodeErrors::Static(err) => writeln!(f, "{}", err),
            HdrEncodeErrors::IoErrors(err) => writeln!(f, "I/O error {:?}", err)
        }
    }
}

impl From<&'static str> for HdrEncodeErrors {
    fn from(value: &'static str) -> Self {
        HdrEncodeErrors::Static(value)
    }
}
impl From<ZByteIoError> for HdrEncodeErrors {
    fn from(value: ZByteIoError) -> Self {
        HdrEncodeErrors::IoErrors(value)
    }
}
