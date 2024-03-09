#![cfg(feature = "std")]

use std::io;
use std::io::{BufRead, BufReader, Read, Seek};

use crate::bytestream::reader::{ZByteIoError, ZSeekFrom};
use crate::bytestream::ZByteReaderTrait;

impl<T> ZByteReaderTrait for std::io::Cursor<T>
where
    T: AsRef<[u8]>
{
    #[inline(always)]
    fn read_byte_no_error(&mut self) -> u8 {
        let byte = *self
            .get_ref()
            .as_ref()
            .get(self.position() as usize)
            .unwrap_or(&0);
        self.set_position(self.position() + 1);
        byte
    }
    #[inline(always)]
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        self.read_exact(buf).map_err(ZByteIoError::from)
    }

    #[inline(always)]
    fn read_const_bytes<const N: usize>(&mut self, buf: &mut [u8; N]) -> Result<(), ZByteIoError> {
        let ref_bytes = self.get_ref().as_ref();

        if self.position() as usize + N <= ref_bytes.len() {
            // we are in bounds
            let position = self.position() as usize;
            if let Some(buf_ref) = ref_bytes.get(position..position + N) {
                buf.copy_from_slice(buf_ref);
                self.set_position((position + N) as u64);
                return Ok(());
            }
        }
        Err(ZByteIoError::Generic("Cannot satisfy read"))
    }

    fn read_const_bytes_no_error<const N: usize>(&mut self, buf: &mut [u8; N]) {
        let ref_bytes = self.get_ref().as_ref();
        if self.position() as usize + N <= ref_bytes.len() {
            // we are in bounds
            let position = self.position() as usize;
            if let Some(buf_ref) = ref_bytes.get(position..position + N) {
                buf.copy_from_slice(buf_ref);
                self.set_position((position + N) as u64);
            }
        }
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

    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        Ok(self.position())
    }

    fn read_remaining(&mut self, sink: &mut Vec<u8>) -> Result<usize, ZByteIoError> {
        self.read_to_end(sink).map_err(ZByteIoError::from)
    }
}

impl<T: io::Read + io::Seek> ZByteReaderTrait for BufReader<T> {
    #[inline(always)]
    fn read_byte_no_error(&mut self) -> u8 {
        let mut buf = [0];
        let _ = self.read(&mut buf);
        buf[0]
    }
    #[inline(always)]
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        self.read_exact(buf).map_err(ZByteIoError::from)
    }

    #[inline]
    fn read_const_bytes<const N: usize>(&mut self, buf: &mut [u8; N]) -> Result<(), ZByteIoError> {
        self.read_exact_bytes(buf)
    }

    fn read_const_bytes_no_error<const N: usize>(&mut self, buf: &mut [u8; N]) {
        let _ = self.read_const_bytes(buf);
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
        self.fill_buf()
            .map(|b| b.is_empty())
            .map_err(ZByteIoError::from)
    }

    #[inline(always)]
    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        self.stream_position().map_err(ZByteIoError::from)
    }

    #[inline(always)]
    fn read_remaining(&mut self, sink: &mut Vec<u8>) -> Result<usize, ZByteIoError> {
        self.read_to_end(sink).map_err(ZByteIoError::from)
    }
}
