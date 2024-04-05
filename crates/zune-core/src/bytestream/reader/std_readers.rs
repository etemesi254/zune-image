#![cfg(feature = "std")]

use std::io;

use crate::bytestream::reader::{ZByteIoError, ZSeekFrom};
use crate::bytestream::ZByteReaderTrait;
// note (cae): If Rust ever stabilizes trait specialization, specialize this for Cursor
impl<T: io::BufRead + io::Seek> ZByteReaderTrait for T {
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
