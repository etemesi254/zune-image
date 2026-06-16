// isobmff/src/error.rs

use core::fmt;

use zune_core::bytestream::ZByteIoError;

pub enum IsoBmffErrors {
    /// Wraps an underlying I/O failure.
    Io(ZByteIoError),

    Generic {
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
