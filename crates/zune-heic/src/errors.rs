// isobmff/src/error.rs

use core::fmt;

use zune_core::bytestream::ZByteIoError;

use crate::bmf_reader::FourCC;
use crate::hevc_decoder::nal_parser::NalError;

pub enum HeicErrors {
    /// Wraps an underlying I/O failure.
    Io(ZByteIoError),

    /// A box header reported a size that is inconsistent with its position
    /// or the surrounding container.
    InvalidBoxSize {
        offset: u64,
        size:   u64
    },

    /// The parser needed more bytes than were available.
    UnexpectedEof {
        offset: u64,
        needed: u64
    },

    /// The four-byte box-type field contained non-printable bytes.
    InvalidBoxType([u8; 4]),

    /// A FullBox carried a `version` value the parser does not handle.
    UnsupportedVersion {
        offset:  u64,
        version: u8
    },

    /// A box payload was shorter than the minimum required by its spec.
    PayloadTooShort {
        box_type: FourCC,
        needed:   usize,
        have:     usize
    },

    /// Any other structural problem found while parsing a specific box.
    ParseError {
        box_type: FourCC,
        msg:      String
    },
    WouldUnderflow {
        a: usize,
        b: usize
    },
    Generic {
        msg: String
    },
    NalErrors(NalError)
}
impl fmt::Display for HeicErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeicErrors::Io(e) => {
                write!(f, "I/O error: {e:?}")
            }
            HeicErrors::InvalidBoxSize { offset, size } => {
                write!(f, "Box at offset {offset} has invalid size {size}")
            }
            HeicErrors::UnexpectedEof { offset, needed } => {
                write!(
                    f,
                    "Unexpected end of data: need {needed} bytes at offset {offset}"
                )
            }
            HeicErrors::InvalidBoxType(bytes) => {
                write!(f, "Box type contains non-ASCII bytes: {bytes:?}")
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
            HeicErrors::WouldUnderflow { a, b } => {
                write!(f, "Would underflow ({a}-{b}) ")
            }
            HeicErrors::Generic { msg } => {
                write!(f, "{msg}")
            }
            HeicErrors::NalErrors(err) => {
                write!(f, "{err}")
            }
        }
    }
}
impl fmt::Debug for HeicErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeicErrors::Io(e) => f.debug_tuple("Io").field(e).finish(),
            HeicErrors::InvalidBoxSize { offset, size } => f
                .debug_struct("InvalidBoxSize")
                .field("offset", offset)
                .field("size", size)
                .finish(),
            HeicErrors::UnexpectedEof { offset, needed } => f
                .debug_struct("UnexpectedEof")
                .field("offset", offset)
                .field("needed", needed)
                .finish(),
            HeicErrors::InvalidBoxType(bytes) => {
                f.debug_tuple("InvalidBoxType").field(bytes).finish()
            }
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
            HeicErrors::NalErrors(msg) => f.debug_tuple("NalErrors").field(msg).finish()
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
