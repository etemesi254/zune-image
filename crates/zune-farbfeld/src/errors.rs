use core::fmt::{Debug, Formatter};

use zune_core::bytestream::ZByteIoError;

pub enum FarbFeldErrors {
    Generic(&'static str),
    IoError(ZByteIoError)
}

impl Debug for FarbFeldErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            FarbFeldErrors::Generic(e) => {
                writeln!(f, "Generic: {e}")
            }
            FarbFeldErrors::IoError(e) => {
                writeln!(f, "IO error: {:?}", e)
            }
        }
    }
}

impl From<ZByteIoError> for FarbFeldErrors {
    fn from(value: ZByteIoError) -> Self {
        FarbFeldErrors::IoError(value)
    }
}
