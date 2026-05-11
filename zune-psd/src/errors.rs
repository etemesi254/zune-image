use std::fmt::{Debug, Formatter};

use crate::constants::{ColorModes, PSD_IDENTIFIER_BE};

/// PSDDecodeErrors that can occur during PSD decoding
pub enum PSDDecodeErrors
{
    WrongMagicBytes(u32),
    UnsupportedFileType(u16),
    UnsupportedChannelCount(u16),
    UnsupportedBitDepth(u16),
    UnsupportedColorFormat(Option<ColorModes>),
    LargeDimensions(usize, usize),
    ZeroDimensions,
    UnknownCompression,
    Generic(&'static str),
    BadRLE
}

impl Debug for PSDDecodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            PSDDecodeErrors::Generic(reason) =>
            {
                write!(f, "{reason}")
            }
            PSDDecodeErrors::WrongMagicBytes(bytes) =>
            {
                write!(
                    f,
                    "Expected {:?} but found  {:?}, not a PSD image",
                    PSD_IDENTIFIER_BE.to_be_bytes(),
                    bytes.to_be_bytes()
                )
            }
            PSDDecodeErrors::UnsupportedFileType(version) =>
            {
                write!(
                    f,
                    "Unsupported file version {version:?}, known versions are 1",
                )
            }
            PSDDecodeErrors::UnsupportedChannelCount(channels) =>
            {
                write!(f, "Unsupported channel count {channels:?}")
            }
            PSDDecodeErrors::UnsupportedBitDepth(depth) =>
            {
                write!(
                    f,
                    "Unsupported bit depth {depth:?}, supported depths are 8 and 16",
                )
            }
            PSDDecodeErrors::UnsupportedColorFormat(color) =>
            {
                if let Some(color) = color
                {
                    write!(
                        f,
                        "Unsupported color format  {color:?}, supported formats RGB only",
                    )
                }
                else
                {
                    write!(f, "Unknown color format")
                }
            }
            PSDDecodeErrors::UnknownCompression =>
            {
                write!(f, "Unknown compression format")
            }
            PSDDecodeErrors::BadRLE =>
            {
                write!(f, "Bad RLE")
            }
            PSDDecodeErrors::LargeDimensions(supported, found) =>
            {
                write!(
                    f,
                    "Too large dimensions, supported {supported} but found {found}",
                )
            }
            PSDDecodeErrors::ZeroDimensions =>
            {
                write!(f, "Zero found where not expected")
            }
        }
    }
}

impl From<&'static str> for PSDDecodeErrors
{
    fn from(r: &'static str) -> Self
    {
        Self::Generic(r)
    }
}
