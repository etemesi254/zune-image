/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use core::fmt::{Debug, Formatter};

use zune_core::bytestream::ZByteIoError;

use crate::constants::{ColorModes, PSD_IDENTIFIER_BE};

/// PSDDecodeErrors that can occur during PSD decoding
pub enum PSDDecodeErrors {
    WrongMagicBytes(u32),
    UnsupportedFileType(u16),
    UnsupportedChannelCount(u16),
    UnsupportedBitDepth(u16),
    UnsupportedColorFormat(Option<ColorModes>),
    LargeDimensions(usize, usize),
    ZeroDimensions,
    UnknownCompression,
    Generic(&'static str),
    IoErrors(ZByteIoError),
    BadRLE
}

impl Debug for PSDDecodeErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            PSDDecodeErrors::Generic(reason) => {
                writeln!(f, "{reason}")
            }
            PSDDecodeErrors::WrongMagicBytes(bytes) => {
                writeln!(
                    f,
                    "Expected {:?} but found  {:?}, not a PSD image",
                    PSD_IDENTIFIER_BE.to_be_bytes(),
                    bytes.to_be_bytes()
                )
            }
            PSDDecodeErrors::UnsupportedFileType(version) => {
                writeln!(
                    f,
                    "Unsupported file version {version:?}, known versions are 1",
                )
            }
            PSDDecodeErrors::UnsupportedChannelCount(channels) => {
                writeln!(f, "Unsupported channel count {channels:?}")
            }
            PSDDecodeErrors::UnsupportedBitDepth(depth) => {
                writeln!(
                    f,
                    "Unsupported bit depth {depth:?}, supported depths are 8 and 16",
                )
            }
            PSDDecodeErrors::UnsupportedColorFormat(color) => {
                if let Some(color) = color {
                    writeln!(
                        f,
                        "Unsupported color format  {color:?}, supported formats RGB,CMYK and Grayscale currently",
                    )
                } else {
                    writeln!(f, "Unknown color format")
                }
            }
            PSDDecodeErrors::UnknownCompression => {
                writeln!(f, "Unknown compression format")
            }
            PSDDecodeErrors::BadRLE => {
                writeln!(f, "Bad RLE")
            }
            PSDDecodeErrors::LargeDimensions(supported, found) => {
                writeln!(
                    f,
                    "Too large dimensions, supported {supported} but found {found}",
                )
            }
            PSDDecodeErrors::ZeroDimensions => {
                writeln!(f, "Zero found where not expected")
            }
            PSDDecodeErrors::IoErrors(e) => {
                writeln!(f, "I/O error :{:?}", e)
            }
        }
    }
}

impl From<&'static str> for PSDDecodeErrors {
    fn from(r: &'static str) -> Self {
        Self::Generic(r)
    }
}

impl From<ZByteIoError> for PSDDecodeErrors {
    fn from(r: ZByteIoError) -> Self {
        Self::IoErrors(r)
    }
}
