use core::fmt::Debug;
use std::fmt::Formatter;

use zune_core::bytestream::ZByteIoError;

pub enum GifDecoderErrors {
    /// File is not a gif
    NotAGif,
    /// A generic error
    Static(&'static str),
    /// To large dimensions for width or height
    TooLargeDimensions(&'static str, usize, usize),
    /// Underlying input output errors
    IoErrors(ZByteIoError),
    /// A calculation that wasn't meant to overflow overflowed
    OverflowError(&'static str),
    /// Too small size
    TooSmallSize(usize, usize)
}
impl Debug for GifDecoderErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GifDecoderErrors::NotAGif => {
                writeln!(f, "Not a gif, magic bytes didn't match")
            }
            GifDecoderErrors::Static(v) => {
                writeln!(f, "{}", v)
            }
            GifDecoderErrors::TooLargeDimensions(a, b, c) => {
                writeln!(
                    f,
                    "Too large dimensions for {a} expected less than {b} but found  {c}"
                )
            }
            Self::OverflowError(err) => {
                writeln!(
                    f,
                    "A calculation that wasn't meant to overflow overflowed :{err}"
                )
            }
            Self::TooSmallSize(at_least, present) => {
                writeln!(f, "Expected a size of {at_least} but found {present}")
            }
            Self::IoErrors(err) => {
                writeln!(f, "{:?}", err)
            }
        }
    }
}

impl From<&'static str> for GifDecoderErrors {
    fn from(value: &'static str) -> Self {
        Self::Static(value)
    }
}

impl From<ZByteIoError> for GifDecoderErrors {
    fn from(value: ZByteIoError) -> Self {
        GifDecoderErrors::IoErrors(value)
    }
}
