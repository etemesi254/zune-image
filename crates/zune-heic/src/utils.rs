use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_isobmff::FourCC;

use crate::errors::HeicErrors;

pub(crate) fn read_sized_int<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, size: u8
) -> Result<u64, HeicErrors> {
    match size {
        0 => Ok(0),
        4 => Ok(u64::from(reader.get_u32_be_err()?)),
        8 => Ok(reader.get_u64_be_err()?),
        _ => Err(HeicErrors::ParseError {
            box_type: FourCC(*b"iloc"),
            msg:      format!("Unsupported variable integer size: {size}")
        })
    }
}

/// A lock around data that may be written from several threads.
///
/// With `std` this is a [`std::sync::Mutex`], so it can be shared between
/// decoding threads. Without `std` decoding is single-threaded, so a
/// [`core::cell::RefCell`] is enough.
pub(crate) struct Lock<T> {
    #[cfg(feature = "std")]
    inner: std::sync::Mutex<T>,
    #[cfg(not(feature = "std"))]
    inner: core::cell::RefCell<T>
}

impl<T> Lock<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            #[cfg(feature = "std")]
            inner: std::sync::Mutex::new(value),
            #[cfg(not(feature = "std"))]
            inner: core::cell::RefCell::new(value)
        }
    }

    /// Run `f` with exclusive access to the data.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        #[cfg(feature = "std")]
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        #[cfg(not(feature = "std"))]
        let mut guard = self.inner.borrow_mut();
        f(&mut guard)
    }
}
