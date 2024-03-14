use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Formatter;

pub(crate) mod no_std_readers;
pub(crate) mod std_readers;
use crate::bytestream::ZByteReaderTrait;

/// Enumeration of possible methods to seek within an I/O object.
///
/// It is analogous to the [SeekFrom](std::io::SeekFrom) in the std library but
/// it's here to allow this to work in no-std crates
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
pub enum ZSeekFrom {
    /// Sets the offset to the provided number of bytes.
    Start(u64),

    /// Sets the offset to the size of this object plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    End(i64),

    /// Sets the offset to the current position plus the specified number of
    /// bytes.
    ///
    /// It is possible to seek beyond the end of an object, but it's an error to
    /// seek before byte 0.
    Current(i64)
}

impl ZSeekFrom {
    /// Convert to [SeekFrom](std::io::SeekFrom) from the `std::io` library
    ///
    /// This is only present when std feature is present
    #[cfg(feature = "std")]
    pub(crate) fn to_std_seek(self) -> std::io::SeekFrom {
        match self {
            ZSeekFrom::Start(pos) => std::io::SeekFrom::Start(pos),
            ZSeekFrom::End(pos) => std::io::SeekFrom::End(pos),
            ZSeekFrom::Current(pos) => std::io::SeekFrom::Current(pos)
        }
    }
}

pub enum ZByteIoError {
    #[cfg(feature = "std")]
    StdIoError(std::io::Error),
    TryFromIntError(core::num::TryFromIntError),
    // requested, read
    NotEnoughBytes(usize, usize),
    NotEnoughBuffer(usize, usize),
    Generic(&'static str),
    SeekError(&'static str),
    SeekErrorOwned(String)
}

impl core::fmt::Debug for ZByteIoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            #[cfg(feature = "std")]
            ZByteIoError::StdIoError(err) => {
                writeln!(f, "Underlying I/O error {}", err)
            }
            ZByteIoError::TryFromIntError(err) => {
                writeln!(f, "Cannot convert to int {}", err)
            }
            ZByteIoError::NotEnoughBytes(expected, found) => {
                writeln!(f, "Not enough bytes, expected {expected} but found {found}")
            }
            ZByteIoError::NotEnoughBuffer(expected, found) => {
                writeln!(
                    f,
                    "Not enough buffer to write {expected} bytes, buffer size is {found}"
                )
            }
            ZByteIoError::Generic(err) => {
                writeln!(f, "Generic I/O error: {err}")
            }
            ZByteIoError::SeekError(err) => {
                writeln!(f, "Seek error: {err}")
            }
            ZByteIoError::SeekErrorOwned(err) => {
                writeln!(f, "Seek error {err}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for ZByteIoError {
    fn from(value: std::io::Error) -> Self {
        ZByteIoError::StdIoError(value)
    }
}

impl From<core::num::TryFromIntError> for ZByteIoError {
    fn from(value: core::num::TryFromIntError) -> Self {
        ZByteIoError::TryFromIntError(value)
    }
}

impl From<&'static str> for ZByteIoError {
    fn from(value: &'static str) -> Self {
        ZByteIoError::Generic(value)
    }
}

pub struct ZReader<T: ZByteReaderTrait> {
    inner:       T,
    temp_buffer: Vec<u8>
}

impl<T: ZByteReaderTrait> ZReader<T> {
    pub fn new(source: T) -> ZReader<T> {
        ZReader {
            inner:       source,
            temp_buffer: vec![]
        }
    }
    /// Destroy this reader returning
    /// the underlying source of the bytes
    /// from which we were decoding

    #[inline(always)]
    pub fn consume(self) -> T {
        self.inner
    }
    #[inline(always)]

    pub fn skip(&mut self, num: usize) -> Result<u64, ZByteIoError> {
        self.inner.z_seek(ZSeekFrom::Current(num as i64))
    }
    #[inline(always)]
    pub fn rewind(&mut self, num: usize) -> Result<u64, ZByteIoError> {
        self.inner.z_seek(ZSeekFrom::Current(-(num as i64)))
    }
    #[inline(always)]

    pub fn seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError> {
        self.inner.z_seek(from)
    }
    #[inline(always)]
    pub fn get_u8(&mut self) -> u8 {
        self.inner.read_byte_no_error()
    }
    #[inline(always)]
    pub fn get_u8_err(&mut self) -> Result<u8, ZByteIoError> {
        let mut buf = [0];
        self.inner.read_exact_bytes(&mut buf)?;
        Ok(buf[0])
    }

    /// Look ahead position bytes and return a reference
    /// to num_bytes from that position, or an error if the
    /// peek would be out of bounds.
    ///
    /// This doesn't increment the position, bytes would have to be discarded
    /// at a later point.
    #[inline]
    pub fn peek_at(&mut self, position: usize, num_bytes: usize) -> Result<&[u8], ZByteIoError> {
        // short circuit for zero
        // important since implementations like File will
        // cause a syscall on skip
        if position != 0 {
            // skip position bytes from start
            self.skip(position)?;
        }
        // resize buffer
        self.temp_buffer.resize(num_bytes, 0);
        // read bytes
        match self.inner.peek_exact_bytes(&mut self.temp_buffer[..]) {
            Ok(_) => {
                // rewind back to where we were
                if position != 0 {
                    self.rewind(position)?;
                }
                Ok(&self.temp_buffer)
            }
            Err(e) => Err(e)
        }
    }
    #[inline(always)]
    pub fn read_fixed_bytes_or_error<const N: usize>(&mut self) -> Result<[u8; N], ZByteIoError> {
        let mut byte_store: [u8; N] = [0; N];
        match self.inner.read_const_bytes(&mut byte_store) {
            Ok(_) => Ok(byte_store),
            Err(e) => Err(e)
        }
    }
    #[inline(always)]
    pub fn get_fixed_bytes_or_zero<const N: usize>(&mut self) -> [u8; N] {
        let mut byte_store: [u8; N] = [0; N];
        self.inner.read_const_bytes_no_error(&mut byte_store);
        byte_store
    }

    pub fn skip_until_false<F: Fn(u8) -> bool>(&mut self, func: F) -> Result<(), ZByteIoError> {
        while !self.inner.is_eof()? {
            let byte = self.get_u8();
            if !(func)(byte) {
                self.rewind(1)?;
                break;
            }
        }
        Ok(())
    }

    #[inline]
    pub fn set_position(&mut self, position: usize) -> Result<(), ZByteIoError> {
        self.seek(ZSeekFrom::Start(position as u64))?;

        Ok(())
    }

    #[inline(always)]
    pub fn eof(&mut self) -> Result<bool, ZByteIoError> {
        self.inner.is_eof()
    }
    #[inline(always)]
    pub fn position(&mut self) -> Result<u64, ZByteIoError> {
        self.inner.z_position()
    }

    pub fn remaining_bytes(&mut self) -> Result<&[u8], ZByteIoError> {
        self.temp_buffer.clear();
        let bytes_read = self.inner.read_remaining(&mut self.temp_buffer)?;
        Ok(&self.temp_buffer[..bytes_read])
    }

    pub fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        self.inner.read_exact_bytes(buf)
    }

    pub fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        self.inner.read_bytes(buf)
    }
}

enum Mode {
    // Big endian
    BE,
    // Little Endian
    LE
}
macro_rules! get_single_type {
    ($name:tt,$name2:tt,$name3:tt,$name4:tt,$name5:tt,$name6:tt,$int_type:tt) => {
        impl<T:ZByteReaderTrait> ZReader<T>
        {
            #[inline(always)]
            fn $name(&mut self, mode: Mode) -> $int_type
            {
                const SIZE_OF_VAL: usize = core::mem::size_of::<$int_type>();

                let mut space = [0; SIZE_OF_VAL];

                self.inner.read_const_bytes_no_error(&mut space);

                match mode {
                    Mode::BE => $int_type::from_be_bytes(space),
                    Mode::LE => $int_type::from_le_bytes(space)
                }
            }

            #[inline(always)]
            fn $name2(&mut self, mode: Mode) -> Result<$int_type, ZByteIoError>
            {
                const SIZE_OF_VAL: usize = core::mem::size_of::<$int_type>();

                let mut space = [0; SIZE_OF_VAL];

                match self.inner.read_const_bytes(&mut space)
                {
                    Ok(_) => match mode {
                        Mode::BE => Ok($int_type::from_be_bytes(space)),
                        Mode::LE => Ok($int_type::from_le_bytes(space))
                    },
                     Err(e) =>  Err(e)
                }
            }
            #[doc=concat!("Read ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," read.")]
            #[inline]
            pub fn $name3(&mut self) -> Result<$int_type, ZByteIoError>
            {
                self.$name2(Mode::BE)
            }

            #[doc=concat!("Read ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," read.")]
            #[inline]
            pub fn $name4(&mut self) -> Result<$int_type, ZByteIoError>
            {
                self.$name2(Mode::LE)
            }
            #[doc=concat!("Read ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Returning 0 if the underlying  buffer does not have enough bytes for a ",stringify!($int_type)," read.")]
            #[inline(always)]
            pub fn $name5(&mut self) -> $int_type
            {
                self.$name(Mode::BE)
            }
            #[doc=concat!("Read ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Returning 0 if the underlying buffer does not have enough bytes for a ",stringify!($int_type)," read.")]
            #[inline(always)]
            pub fn $name6(&mut self) -> $int_type
            {
                self.$name(Mode::LE)
            }
        }
    };
}

get_single_type!(
    get_u16_inner_or_default,
    get_u16_inner_or_die,
    get_u16_be_err,
    get_u16_le_err,
    get_u16_be,
    get_u16_le,
    u16
);
get_single_type!(
    get_u32_inner_or_default,
    get_u32_inner_or_die,
    get_u32_be_err,
    get_u32_le_err,
    get_u32_be,
    get_u32_le,
    u32
);
get_single_type!(
    get_u64_inner_or_default,
    get_u64_inner_or_die,
    get_u64_be_err,
    get_u64_le_err,
    get_u64_be,
    get_u64_le,
    u64
);

#[cfg(feature = "std")]
impl<T> std::io::Read for ZReader<T>
where
    T: ZByteReaderTrait
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::io::ErrorKind;
        self.read_bytes(buf)
            .map_err(|e| std::io::Error::new(ErrorKind::Other, format!("{:?}", e)))
    }
}
