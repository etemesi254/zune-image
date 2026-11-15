use alloc::string::String;
// isobmff/src/error.rs

use core::fmt;

use zune_core::bytestream::ZByteIoError;
use zune_isobmff::{IsoBmffErrors, FourCC};

use crate::hevc_decoder::nal_parser::NalError;

/// Errors returned while decoding a HEIF/HEIC file.
pub enum HeicErrors {
    /// Wraps an underlying I/O failure.
    Io(ZByteIoError),

    /// The parser needed more bytes than were available.
    UnexpectedEof {
        /// Position in the file
        offset: u64,
        /// Bytes needed
        needed: u64
    },

    /// A FullBox carried a `version` value the parser does not handle.
    UnsupportedVersion {
        /// Position of the box in the file
        offset:  u64,
        /// The version found
        version: u8
    },

    /// A box payload was shorter than the minimum required by its spec.
    PayloadTooShort {
        /// Type of the box
        box_type: FourCC,
        /// Bytes needed
        needed:   usize,
        /// Bytes available
        have:     usize
    },

    /// Any other structural problem found while parsing a specific box.
    ParseError {
        /// Type of the box
        box_type: FourCC,
        /// What is wrong
        msg:      String
    },
    /// A size computation `a - b` would have gone below zero: the file is
    /// inconsistent.
    WouldUnderflow {
        /// Left-hand side
        a: usize,
        /// Right-hand side
        b: usize
    },
    /// Any other problem, described by `msg`: unsupported features, limits
    /// exceeded, inconsistent structures.
    Generic {
        /// What is wrong
        msg: String
    },
    /// An error in the HEVC bitstream of the image data.
    NalErrors(NalError),
    /// An error reading an ISO-BMFF box header.
    HeaderError(IsoBmffErrors),
    /// The buffer given to [`decode_into`](crate::HeifDecoder::decode_into)
    /// is too small: `(expected, found)` bytes
    TooSmallOutput(usize, usize),
}
impl fmt::Display for HeicErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeicErrors::Io(e) => {
                write!(f, "I/O error: {e:?}")
            }
            HeicErrors::UnexpectedEof { offset, needed } => {
                write!(
                    f,
                    "Unexpected end of data: need {needed} bytes at offset {offset}"
                )
            }
            HeicErrors::UnsupportedVersion { offset, version } => {
                write!(
                    f,
                    "FullBox at offset {offset} has unsupported version {version}"
                )
            }
            HeicErrors::PayloadTooShort {
                box_type,
                needed,
                have
            } => {
                write!(
                    f,
                    "Payload too short for box '{box_type}': need {needed}, have {have}"
                )
            }
            HeicErrors::ParseError { box_type, msg } => {
                write!(f, "Parse error in box '{box_type}': {msg}")
            }
            HeicErrors::TooSmallOutput(expected, found) => {
                write!(
                    f,
                    "Too small output, expected buffer with at least {expected} bytes but got one with {found} bytes"
                )
            }
            HeicErrors::WouldUnderflow { a, b } => {
                write!(f, "Would underflow ({a}-{b}) ")
            }
            HeicErrors::Generic { msg } => {
                write!(f, "{msg}")
            }
            HeicErrors::NalErrors(err) => {
                write!(f, "{err}")
            }
            HeicErrors::HeaderError(err) => {
                write!(f, "{err}")
            }
        }
    }
}
impl fmt::Debug for HeicErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeicErrors::Io(e) => f.debug_tuple("Io").field(e).finish(),
            HeicErrors::UnexpectedEof { offset, needed } => f
                .debug_struct("UnexpectedEof")
                .field("offset", offset)
                .field("needed", needed)
                .finish(),
            HeicErrors::UnsupportedVersion { offset, version } => f
                .debug_struct("UnsupportedVersion")
                .field("offset", offset)
                .field("version", version)
                .finish(),
            HeicErrors::PayloadTooShort {
                box_type,
                needed,
                have
            } => f
                .debug_struct("PayloadTooShort")
                .field("box_type", box_type)
                .field("needed", needed)
                .field("have", have)
                .finish(),
            HeicErrors::ParseError { box_type, msg } => f
                .debug_struct("ParseError")
                .field("box_type", box_type)
                .field("msg", msg)
                .finish(),

            HeicErrors::WouldUnderflow { a, b } => {
                write!(f, "Would underflow ({a}-{b}) ")
            }
            HeicErrors::Generic { msg } => f.debug_tuple("Generic").field(msg).finish(),
            HeicErrors::NalErrors(msg) => f.debug_tuple("NalErrors").field(msg).finish(),
            HeicErrors::HeaderError(err) => f.debug_tuple("HeaderError").field(err).finish(),
            HeicErrors::TooSmallOutput(expected, found) => f
                .debug_tuple("TooSmallOutput")
                .field(expected)
                .field(found)
                .finish(),
        }
    }
}
impl core::error::Error for HeicErrors {}

impl From<ZByteIoError> for HeicErrors {
    fn from(e: ZByteIoError) -> Self {
        HeicErrors::Io(e)
    }
}

impl From<NalError> for HeicErrors {
    fn from(value: NalError) -> Self {
        Self::NalErrors(value)
    }
}

impl From<IsoBmffErrors> for HeicErrors {
    fn from(value: IsoBmffErrors) -> Self {
        Self::HeaderError(value)
    }
}
