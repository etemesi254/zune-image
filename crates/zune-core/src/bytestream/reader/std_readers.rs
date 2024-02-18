#![cfg(feature = "std")]

use std::io;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

use crate::bytestream::reader::{ZByteIoError, ZSeekFrom};
use crate::bytestream::ZByteIoTrait;

impl<T> ZByteIoTrait for std::io::Cursor<T>
where
    T: AsRef<[u8]>
{
    #[inline(always)]
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        self.read_exact(buf).map_err(ZByteIoError::from)
    }

    #[inline(always)]
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        self.read(buf).map_err(ZByteIoError::from)
    }

    #[inline(always)]
    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        // first read bytes to the buffer
        let bytes_read = self.read_bytes(buf)?;
        let converted = -i64::try_from(bytes_read).map_err(ZByteIoError::from)?;
        self.seek(std::io::SeekFrom::Current(converted))
            .map_err(ZByteIoError::from)?;

        Ok(bytes_read)
    }

    #[inline(always)]
    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        // first read bytes to the buffer
        self.read_exact_bytes(buf)?;
        let converted = -i64::try_from(buf.len()).map_err(ZByteIoError::from)?;
        self.seek(std::io::SeekFrom::Current(converted))
            .map_err(ZByteIoError::from)?;
        Ok(())
    }

    #[inline(always)]
    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError> {
        self.seek(from.to_std_seek()).map_err(ZByteIoError::from)
    }

    #[inline(always)]
    fn is_eof(&mut self) -> Result<bool, ZByteIoError> {
        Ok(self.position() as usize >= self.get_ref().as_ref().len())
    }

    #[inline(always)]

    fn z_size(&mut self) -> Result<i64, ZByteIoError> {
        Ok(self.get_ref().as_ref().len() as i64)
    }
    fn name(&self) -> &'static str {
        "Cursor<T>"
    }

    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        Ok(self.position())
    }

    fn read_remaining(&mut self, sink: &mut Vec<u8>) -> Result<usize, ZByteIoError> {
        self.read_to_end(sink).map_err(ZByteIoError::from)
    }
}

impl<T: io::Read + io::Seek> ZByteIoTrait for BufReader<T> {
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        self.read_exact(buf).map_err(ZByteIoError::from)
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        self.read(buf).map_err(ZByteIoError::from)
    }

    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        // first read bytes to the buffer
        let bytes_read = self.read_bytes(buf)?;
        let converted = -i64::try_from(bytes_read).map_err(ZByteIoError::from)?;
        self.seek(std::io::SeekFrom::Current(converted))
            .map_err(ZByteIoError::from)?;

        Ok(bytes_read)
    }

    #[inline]

    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        // first read bytes to the buffer
        self.read_exact_bytes(buf)?;
        let converted = -i64::try_from(buf.len()).map_err(ZByteIoError::from)?;
        self.seek(std::io::SeekFrom::Current(converted))
            .map_err(ZByteIoError::from)?;

        Ok(())
    }

    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError> {
        self.seek(from.to_std_seek()).map_err(ZByteIoError::from)
    }

    fn is_eof(&mut self) -> Result<bool, ZByteIoError> {
        self.fill_buf()
            .map(|b| b.is_empty())
            .map_err(ZByteIoError::from)
    }

    fn z_size(&mut self) -> Result<i64, ZByteIoError> {
        let old_pos = self.stream_position()?;
        let len = self.seek(SeekFrom::End(0))?;

        // Avoid seeking a third time when we were already at the end of the
        // stream. The branch is usually way cheaper than a seek operation.
        if old_pos != len {
            self.seek(SeekFrom::Start(old_pos))?;
        }

        Ok(len as i64)
    }

    fn name(&self) -> &'static str {
        "BufReader<T>"
    }

    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        self.stream_position().map_err(ZByteIoError::from)
    }

    fn read_remaining(&mut self, sink: &mut Vec<u8>) -> Result<usize, ZByteIoError> {
        self.read_to_end(sink).map_err(ZByteIoError::from)
    }
}
