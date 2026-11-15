//! Errors returned while parsing boxes

use alloc::string::String;
use core::fmt;

use zune_core::bytestream::ZByteIoError;

/// Errors returned by this crate.
pub enum IsoBmffErrors {
    /// Reading failed, e.g. the data ends in the middle of a box header.
    Io(ZByteIoError),

    /// Any other error.
    Generic {
        /// What went wrong
        msg: String,
    },
}

impl fmt::Display for IsoBmffErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsoBmffErrors::Io(e) => {
                write!(f, "I/O error: {e:?}")
            }
            IsoBmffErrors::Generic { msg } => {
                write!(f, "{msg}")
            }
        }
    }
}

impl fmt::Debug for IsoBmffErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsoBmffErrors::Io(e) => f.debug_tuple("Io").field(e).finish(),
            IsoBmffErrors::Generic { msg } => f.debug_tuple("Generic").field(msg).finish(),
        }
    }
}

impl core::error::Error for IsoBmffErrors {}

impl From<ZByteIoError> for IsoBmffErrors {
    fn from(e: ZByteIoError) -> Self {
        IsoBmffErrors::Io(e)
    }
}
