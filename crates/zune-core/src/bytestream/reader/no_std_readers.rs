use crate::bytestream::reader::{ZByteIoError, ZSeekFrom};
use crate::bytestream::ZByteReaderTrait;
/// Wraps an in memory buffer providing it with a `Seek` method
/// but works in `no_std` environments
///
/// `std::io::Cursor` is available in std environments, but we also need support
/// for `no_std` environments so this serves as a drop in replacement
pub struct ZCursor<T: AsRef<[u8]>> {
    stream:   T,
    position: usize
}

impl<T: AsRef<[u8]>> ZCursor<T> {
    pub fn new(buffer: T) -> ZCursor<T> {
        ZCursor {
            stream:   buffer,
            position: 0
        }
    }
}

impl<T: AsRef<[u8]>> ZCursor<T> {
    #[inline]
    pub fn skip(&mut self, num: usize) {
        // Can this overflow ??
        self.position = self.position.wrapping_add(num);
    }
    #[inline]
    pub fn rewind(&mut self, num: usize) {
        self.position = self.position.checked_sub(num).unwrap();
    }
}

impl<T: AsRef<[u8]>> ZByteReaderTrait for ZCursor<T> {
    #[inline(always)]
    fn read_byte_no_error(&mut self) -> u8 {
        let byte = self.stream.as_ref().get(self.position).unwrap_or(&0);
        self.position += 1;
        *byte
    }
    #[inline(always)]
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        let bytes_read = self.read_bytes(buf)?;
        if bytes_read != buf.len() {
            // restore read to initial position it was in.
            self.rewind(bytes_read);
            // not all bytes were read.
            return Err(ZByteIoError::NotEnoughBytes(bytes_read, buf.len()));
        }
        Ok(())
    }

    fn read_const_bytes<const N: usize>(&mut self, buf: &mut [u8; N]) -> Result<(), ZByteIoError> {
        if self.position + N <= self.stream.as_ref().len() {
            // we are in bounds
            let reference = self.stream.as_ref();
            let position = self.position;
            if let Some(buf_ref) = reference.get(position..position + N) {
                self.position += N;
                buf.copy_from_slice(buf_ref);
                return Ok(());
            }
        }
        Err(ZByteIoError::Generic("Cannot satisfy read"))
    }

    fn read_const_bytes_no_error<const N: usize>(&mut self, buf: &mut [u8; N]) {
        if self.position + N <= self.stream.as_ref().len() {
            // we are in bounds
            let reference = self.stream.as_ref();
            let position = self.position;
            if let Some(buf_ref) = reference.get(position..position + N) {
                self.position += N;
                buf.copy_from_slice(buf_ref);
            }
        }
    }

    #[inline(always)]
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        let len = self.peek_bytes(buf)?;
        self.skip(len);
        Ok(len)
    }

    #[inline(always)]
    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        let stream_end = self.stream.as_ref().len();

        let start = core::cmp::min(self.position, stream_end);
        let end = core::cmp::min(self.position + buf.len(), stream_end);

        let slice = self.stream.as_ref().get(start..end).unwrap();
        buf[..slice.len()].copy_from_slice(slice);
        let len = slice.len();

        Ok(len)
    }

    #[inline(always)]
    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        self.read_exact_bytes(buf)?;
        self.rewind(buf.len());
        Ok(())
    }

    #[inline(always)]
    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError> {
        let (base_pos, offset) = match from {
            ZSeekFrom::Start(n) => {
                self.position = n as usize;
                return Ok(n);
            }
            ZSeekFrom::End(n) => (self.stream.as_ref().len(), n as isize),
            ZSeekFrom::Current(n) => (self.position, n as isize)
        };
        match base_pos.checked_add_signed(offset) {
            Some(n) => {
                self.position = n;
                Ok(self.position as u64)
            }
            None => Err(ZByteIoError::SeekError("Negative seek"))
        }
    }

    #[inline(always)]
    fn is_eof(&mut self) -> Result<bool, ZByteIoError> {
        Ok(self.position >= self.stream.as_ref().len())
    }
    #[inline(always)]
    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        Ok(self.position as u64)
    }

    fn read_remaining(&mut self, sink: &mut alloc::vec::Vec<u8>) -> Result<usize, ZByteIoError> {
        let start = self.position;
        let end = self.stream.as_ref().len();
        match self.stream.as_ref().get(start..end) {
            None => {
                return Err(ZByteIoError::Generic(
                    "Somehow read remaining couldn't satisfy it's invariants"
                ))
            }
            Some(e) => {
                sink.extend_from_slice(e);
            }
        }
        self.skip(end - start);
        Ok(end - start)
    }
}

#[cfg(feature = "std")]
impl<T: AsRef<[u8]>> std::io::Seek for ZCursor<T> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let (base_pos, offset) = match pos {
            std::io::SeekFrom::Start(n) => {
                self.position = n as usize;
                return Ok(n);
            }
            std::io::SeekFrom::End(n) => (self.stream.as_ref().len(), n as isize),
            std::io::SeekFrom::Current(n) => (self.position, n as isize)
        };
        match base_pos.checked_add_signed(offset) {
            Some(n) => {
                self.position = n;
                Ok(self.position as u64)
            }
            None => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Negative seek"
            ))
        }
    }
}
//
// #[cfg(feature = "std")]
// impl<T: AsRef<[u8]>> std::io::Read for ZCursor<T> {
//     fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
//         self.read_bytes(buf).map_err(|x|{ std::io::Error::new()})
//     }
// }
