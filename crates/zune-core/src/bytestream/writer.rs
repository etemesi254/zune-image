use std::io::Write;
use std::mem::size_of;

enum Mode
{
    // Big endian
    BE,
    // Little Endian
    LE
}

static ERROR_MSG: &str = "No more space";

/// Encapsulates a simple Byte writer with
/// support for Endian aware writes
pub struct ZByteWriter<'a>
{
    buffer:   &'a mut [u8],
    position: usize
}

impl<'a> Write for ZByteWriter<'a>
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize>
    {
        let min = buf.len().min(self.bytes_left());
        // write
        self.buffer[self.position..self.position + min].copy_from_slice(&buf[0..min]);
        self.position += min;

        Ok(min)
    }

    fn flush(&mut self) -> std::io::Result<()>
    {
        // no need to do anything
        Ok(())
    }
}

impl<'a> ZByteWriter<'a>
{
    /// Create a new writer for the stream
    pub fn new(data: &'a mut [u8]) -> ZByteWriter<'a>
    {
        ZByteWriter {
            buffer:   data,
            position: 0
        }
    }
    /// Return number of unwritten bytes in this stream
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut storage = [0;10];
    ///
    /// let writer = ZByteWriter::new(&mut storage);
    /// assert_eq!(writer.bytes_left(),10); // no bytes were written
    /// ```
    pub const fn bytes_left(&self) -> usize
    {
        self.buffer.len().saturating_sub(self.position)
    }

    /// Return the number of bytes the writer has written
    ///
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut stream = ZByteWriter::new(&mut []);
    /// assert_eq!(stream.position(),0);
    /// ```
    pub const fn position(&self) -> usize
    {
        self.position
    }

    /// Write a single byte into the bytestream or error out
    /// if there is not enough space
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut buf = [0;10];
    /// let mut stream  =  ZByteWriter::new(&mut buf);
    /// assert!(stream.write_u8_err(34).is_ok());
    /// ```
    /// No space
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut stream = ZByteWriter::new(&mut []);
    /// assert!(stream.write_u8_err(32).is_err());
    /// ```
    ///
    pub fn write_u8_err(&mut self, byte: u8) -> Result<(), &'static str>
    {
        match self.buffer.get_mut(self.position)
        {
            Some(m_byte) =>
            {
                self.position += 1;
                *m_byte = byte;

                Ok(())
            }
            None => Err(ERROR_MSG)
        }
    }

    /// Write a single byte in the stream or don't write
    /// anything if the buffer is full and cannot support the byte read
    ///
    /// Should be combined with [`has`](Self::has)
    pub fn write_u8(&mut self, byte: u8)
    {
        if let Some(m_byte) = self.buffer.get_mut(self.position)
        {
            self.position += 1;
            *m_byte = byte;
        }
    }
    /// Check if the byte writer can support
    /// the following write
    ///
    /// # Example
    /// ```
    /// use zune_core::bytestream::ZByteWriter;
    /// let mut data = [0;10];
    /// let mut stream = ZByteWriter::new(&mut data);
    /// assert!(stream.has(5));
    /// assert!(!stream.has(100));
    /// ```
    pub const fn has(&self, bytes: usize) -> bool
    {
        self.position.saturating_add(bytes) <= self.buffer.len()
    }
}

macro_rules! write_single_type {
    ($name:tt,$name2:tt,$name3:tt,$name4:tt,$name5:tt,$name6:tt,$int_type:tt) => {
        impl<'a> ZByteWriter<'a>
        {
            #[inline(always)]
            fn $name(&mut self, byte: $int_type, mode: Mode) -> Result<(), &'static str>
            {
                const SIZE: usize = size_of::<$int_type>();

                match self.buffer.get_mut(self.position..self.position + SIZE)
                {
                    Some(m_byte) =>
                    {
                        self.position += SIZE;
                        // get bits, depending on mode.
                        // This should be inlined and not visible in
                        // the generated binary since mode is a compile
                        // time constant.
                        let bytes = match mode
                        {
                            Mode::BE => byte.to_be_bytes(),
                            Mode::LE => byte.to_le_bytes()
                        };

                        m_byte.copy_from_slice(&bytes);

                        Ok(())
                    }
                    None => Err(ERROR_MSG)
                }
            }
            #[inline(always)]
            fn $name2(&mut self, byte: $int_type, mode: Mode)
            {
                const SIZE: usize = size_of::<$int_type>();

                if let Some(m_byte) = self.buffer.get_mut(self.position..self.position + SIZE)
                {
                    self.position += SIZE;
                    // get bits, depending on mode.
                    // This should be inlined and not visible in
                    // the generated binary since mode is a compile
                    // time constant.
                    let bytes = match mode
                    {
                        Mode::BE => byte.to_be_bytes(),
                        Mode::LE => byte.to_le_bytes()
                    };

                    m_byte.copy_from_slice(&bytes);
                }
            }

            #[doc=concat!("Write ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," write.")]
            #[inline]
            pub fn $name3(&mut self, byte: $int_type) -> Result<(), &'static str>
            {
                self.$name(byte, Mode::BE)
            }

            #[doc=concat!("Write ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Returning an error if the underlying buffer cannot support a ",stringify!($int_type)," write.")]
            #[inline]
            pub fn $name4(&mut self, byte: $int_type) -> Result<(), &'static str>
            {
                self.$name(byte, Mode::LE)
            }

            #[doc=concat!("Write ",stringify!($int_type)," as a big endian integer")]
            #[doc=concat!("Or don't write anything if the reader cannot support a ",stringify!($int_type)," write.")]
            #[doc=concat!("\nShould be combined with the [`has`](Self::has) method to ensure a write succeeds")]
            #[inline]
            pub fn $name5(&mut self, byte: $int_type)
            {
                self.$name2(byte, Mode::BE)
            }
            #[doc=concat!("Write ",stringify!($int_type)," as a little endian integer")]
            #[doc=concat!("Or don't write anything if the reader cannot support a ",stringify!($int_type)," write.")]
            #[doc=concat!("Should be combined with the [`has`](Self::has) method to ensure a write succeeds")]
            #[inline]
            pub fn $name6(&mut self, byte: $int_type)
            {
                self.$name2(byte, Mode::LE)
            }
        }
    };
}

write_single_type!(
    write_u64_inner_or_die,
    write_u64_inner_or_none,
    write_u64_be_err,
    write_u64_le_err,
    write_u64_be,
    write_u64_le,
    u64
);

write_single_type!(
    write_u32_inner_or_die,
    write_u32_inner_or_none,
    write_u32_be_err,
    write_u32_le_err,
    write_u32_be,
    write_u32_le,
    u32
);

write_single_type!(
    write_u16_inner_or_die,
    write_u16_inner_or_none,
    write_u16_be_err,
    write_u16_le_err,
    write_u16_be,
    write_u16_le,
    u16
);
