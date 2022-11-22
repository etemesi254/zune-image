#![cfg(feature = "bytestream")]

use std::cmp::min;
use std::io::Read;

static ERROR_MSG: &str = "No more bytes";

/// An encapsulation of a byte stream reader
///
/// This provides an interface similar to [std::io::Cursor]
/// but with the exception that this doesn't use generics and
/// it provides fine grained options for reading different integer data types from
/// the underlying buffer.
///
/// There are two variants mainly error and non error variants,
/// the error variants are useful for cases where you need bytes
/// from the underlying stream, and cannot do with zero result.
/// the non error variants are useful when you may have proved data already exists
/// eg by using [`has`] method or you are okay with returning zero if the underlying
/// buffer has been completely read.
///
/// [std::io::Cursor]: std::io::Cursor
/// [`has`]: Self::has
pub struct ZByteReader<'a>
{
    /// Data stream
    stream:   &'a [u8],
    position: usize
}
enum Mode
{
    // Big endian
    BE,
    // Little Endian
    LE
}

impl<'a> ZByteReader<'a>
{
    /// Create a new instance of the byte stream
    pub const fn new(buf: &'a [u8]) -> ZByteReader<'a>
    {
        ZByteReader {
            stream:   buf,
            position: 0
        }
    }
    /// Skip `num` bytes ahead of the stream.
    pub fn skip(&mut self, num: usize)
    {
        // Can this overflow ??
        self.position = self.position.wrapping_add(num);
    }
    /// Undo a buffer read by moving the position pointer `num`
    /// bytes behind.
    ///
    /// This operation will saturate at zero
    pub fn rewind(&mut self, num: usize)
    {
        self.position = self.position.saturating_sub(num);
    }

    /// Return whether the underlying buffer
    /// has `num` bytes available for reading
    #[inline]
    pub const fn has(&self, num: usize) -> bool
    {
        self.position + num <= self.stream.len()
    }
    /// Get number of bytes available in the stream
    #[inline]
    pub const fn get_bytes_left(&self) -> usize
    {
        // Must be saturating to prevent underflow
        self.stream.len().saturating_sub(self.position)
    }
    /// Get length of the underlying buffer.
    ///
    /// To get the number of bytes left in the buffer,
    /// use [remaining] method
    ///
    /// [remaining]: Self::remaining
    #[inline]
    pub const fn len(&self) -> usize
    {
        self.stream.len()
    }
    /// Return true if the underlying buffer stream is empty
    #[inline]
    pub const fn is_empty(&self) -> bool
    {
        self.stream.len() == 0
    }
    /// Get current position of the buffer.
    #[inline]
    pub const fn get_position(&self) -> usize
    {
        self.position
    }
    /// Return true whether or not we read to the end of the
    /// buffer and have no more bytes left.
    #[inline]
    pub const fn eof(&self) -> bool
    {
        self.position >= self.len()
    }
    /// Get number of bytes unread inside this
    /// stream.
    ///
    /// To get the length of the underlying stream,
    /// use [len] method
    ///
    /// [len]: Self::len()
    #[inline]
    pub const fn remaining(&self) -> usize
    {
        self.stream.len().saturating_sub(self.position)
    }
    /// Get a part of the bytestream as a reference.
    ///
    /// This increments the position to point past the bytestream
    /// if position+num is in bounds
    pub fn get_as_ref(&mut self, num: usize) -> Result<&'a [u8], &'static str>
    {
        match self.stream.get(self.position..self.position + num)
        {
            Some(bytes) =>
            {
                self.position += num;
                Ok(bytes)
            }
            None => Err(ERROR_MSG)
        }
    }
    /// Look ahead position bytes and return a reference
    /// to num_bytes from that position, or an error if the
    /// peek would be out of bounds.
    ///
    /// This doesn't increment the position, bytes would have to be discarded
    /// at a later point.
    #[inline]
    pub fn peek_at(&self, position: usize, num_bytes: usize) -> Result<&'a [u8], &'static str>
    {
        let start = self.position + position;
        let end = self.position + position + num_bytes;

        match self.stream.get(start..end)
        {
            Some(bytes) => Ok(bytes),
            None => Err(ERROR_MSG)
        }
    }
    #[inline]
    /// Skip bytes until a condition becomes false
    pub fn skip_until_false<F: Fn(u8) -> bool>(&mut self, func: F)
    {
        // iterate until we have no more bytes
        while self.has(1)
        {
            // get a byte from stream
            let byte = self.get_u8();

            if !(func)(byte)
            {
                // function returned false meaning we stop skipping
                self.rewind(1);
                break;
            }
        }
    }
}

impl<'a> Read for ZByteReader<'a>
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>
    {
        let buf_length = buf.len();
        let start = self.position;
        let end = min(self.len(), self.position + buf_length);
        let diff = end - start;

        buf[0..diff].copy_from_slice(&self.stream[start..end]);

        self.skip(diff);

        Ok(diff)
    }
}

macro_rules! get_single_type {
    ($name:tt,$name2:tt,$name3:tt,$name4:tt,$name5:tt,$name6:tt,$int_type:tt) => {
        impl<'a> ZByteReader<'a>
        {
            #[inline(always)]
            fn $name(&mut self, mode: Mode) -> $int_type
            {
                const SIZE_OF_VAL: usize = core::mem::size_of::<$int_type>();

                let mut space = [0; SIZE_OF_VAL];

                match self.stream.get(self.position..self.position + SIZE_OF_VAL)
                {
                    Some(position) =>
                    {
                        space.copy_from_slice(position);
                        self.position += SIZE_OF_VAL;

                        match mode
                        {
                            Mode::LE => $int_type::from_le_bytes(space),
                            Mode::BE => $int_type::from_be_bytes(space),
                        }
                    }
                    None => 0,
                }
            }

            #[inline(always)]
            fn $name2(&mut self, mode: Mode) -> Result<$int_type, &'static str>
            {
                const SIZE_OF_VAL: usize = core::mem::size_of::<$int_type>();

                let mut space = [0; SIZE_OF_VAL];

                match self.stream.get(self.position..self.position + SIZE_OF_VAL)
                {
                    Some(position) =>
                    {
                        space.copy_from_slice(position);
                        self.position += SIZE_OF_VAL;

                        match mode
                        {
                            Mode::LE => Ok($int_type::from_le_bytes(space)),
                            Mode::BE => Ok($int_type::from_be_bytes(space)),
                        }
                    }
                    None => Err(ERROR_MSG),
                }
            }
            #[doc=concat!("Read ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," read.")]
            #[inline]
            pub fn $name3(&mut self) -> Result<$int_type, &'static str>
            {
                self.$name2(Mode::BE)
            }

            #[doc=concat!("Read ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," read.")]
            #[inline]
            pub fn $name4(&mut self) -> Result<$int_type, &'static str>
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
// U8 implementation
// The benefit of our own unrolled u8 impl instead of macros is that this is sometimes used in some
// impls and is called multiple times, e.g jpeg during huffman decoding.
// we can make some functions leaner like get_u8 is branchless
impl<'a> ZByteReader<'a>
{
    /// Retrieve a byte from the underlying stream
    /// returning 0 if there are no more bytes available
    ///
    /// This means 0 might indicate a bit or an end of stream, but
    /// this is useful for some scenarios where one needs a byte.
    ///
    /// For the panicking one, see [`get_u8_err`]
    ///
    /// [`get_u8_err`]: Self::get_u8_err
    #[inline(always)]
    pub fn get_u8(&mut self) -> u8
    {
        let byte = *self.stream.get(self.position).unwrap_or(&0);

        self.position += usize::from(self.position < self.len());
        byte
    }

    /// Retrieve a byte from the underlying stream
    /// returning an error if there are no more bytes available
    ///
    /// For the non panicking one, see [`get_u8`]
    ///
    /// [`get_u8`]: Self::get_u8
    #[inline(always)]
    pub fn get_u8_err(&mut self) -> Result<u8, &'static str>
    {
        match self.stream.get(self.position)
        {
            Some(byte) =>
            {
                self.position += 1;
                Ok(*byte)
            }
            None => Err(ERROR_MSG)
        }
    }
}

// u16,u32,u64 -> macros
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
