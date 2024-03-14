/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(clippy::uninlined_format_args)]

use core::fmt::{Debug, Formatter};

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteIoError;
use zune_core::colorspace::ColorSpace;

const MAX_DIMENSIONS: usize = 1 << 30;

/// Errors that may arise during encoding
pub enum JxlEncodeErrors {
    /// One of the dimensions is less than 2
    ZeroDimension(&'static str),
    /// The colorspace of the image isn't supported by
    /// the library
    UnsupportedColorspace(ColorSpace),
    /// Image depth isn't supported by the library
    UnsupportedDepth(BitDepth),
    /// A given width or height is too big to be encoded
    TooLargeDimensions(usize),
    /// Mismatch in length expected vs what was found
    LengthMismatch(usize, usize),
    /// Generic error
    Generic(&'static str),

    IoErrors(ZByteIoError)
}

pub const SUPPORTED_COLORSPACES: [ColorSpace; 4] = [
    ColorSpace::Luma,
    ColorSpace::LumaA,
    ColorSpace::RGBA,
    ColorSpace::RGB
];
pub const SUPPORTED_DEPTHS: [BitDepth; 2] = [BitDepth::Eight, BitDepth::Sixteen];

impl Debug for JxlEncodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            JxlEncodeErrors::ZeroDimension(param) => writeln!(f, "The {param} is less than 2"),
            JxlEncodeErrors::UnsupportedColorspace(color) => writeln!(
                f,
                "JXL encoder cannot encode images in colorspace {color:?}, supported ones are {:?}",
                SUPPORTED_COLORSPACES
            ),
            JxlEncodeErrors::UnsupportedDepth(depth) => {
                writeln!(
                    f,
                    "JXL encoder cannot encode images in depth {depth:?},supported ones are {:?}",
                    SUPPORTED_DEPTHS
                )
            }
            JxlEncodeErrors::TooLargeDimensions(value) => {
                writeln!(
                        f,
                        "Too large dimensions {value} greater than supported dimensions {MAX_DIMENSIONS}"
                    )
            }
            JxlEncodeErrors::LengthMismatch(expected, found) => {
                writeln!(f, "Expected array of length {expected} but found {found}")
            }
            JxlEncodeErrors::Generic(msg) => {
                writeln!(f, "{}", msg)
            }
            JxlEncodeErrors::IoErrors(e) => {
                writeln!(f, "I/O error {:?}", e)
            }
        }
    }
}

impl From<ZByteIoError> for JxlEncodeErrors {
    fn from(value: ZByteIoError) -> Self {
        JxlEncodeErrors::IoErrors(value)
    }
}
