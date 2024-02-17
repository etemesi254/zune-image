use crate::bytestream::{ZByteIoError, ZByteIoTrait, ZReaderTrait, ZSeekFrom};

pub struct ZByteBuffer<T: ZReaderTrait> {
    stream:   T,
    position: usize
}

impl<T: ZReaderTrait> ZByteBuffer<T> {
    pub fn new(buffer: T) -> ZByteBuffer<T> {
        ZByteBuffer {
            stream:   buffer,
            position: 0
        }
    }
}

impl<T: ZReaderTrait> ZByteBuffer<T> {
    #[inline]
    pub fn skip(&mut self, num: usize) {
        // Can this overflow ??
        self.position = self.position.wrapping_add(num);
    }
    #[inline]
    pub fn rewind(&mut self, num: usize) {
        self.position = self.position.saturating_sub(num);
    }
}

impl<T: ZReaderTrait> ZByteIoTrait for ZByteBuffer<T> {
    #[inline(always)]
    fn read_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        let bytes_read = self.read_bytes(buf)?;
        if bytes_read != buf.len() {
            // not all bytes were read.
            return Err(ZByteIoError::NotEnoughBytes(bytes_read, buf.len()));
        }
        Ok(())
    }
    #[inline(always)]
    fn read_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        let start = core::cmp::min(self.position, self.stream.get_len());
        let end = core::cmp::min(self.position + buf.len(), self.stream.get_len());

        let slice = self.stream.get_slice(start..end).unwrap();
        buf[..slice.len()].copy_from_slice(slice);

        self.skip(end - start);

        Ok(end - start)
    }

    #[inline(always)]
    fn peek_bytes(&mut self, buf: &mut [u8]) -> Result<usize, ZByteIoError> {
        // read bytes
        let bytes_read = self.read_bytes(buf)?;
        // rewind
        self.rewind(bytes_read);

        Ok(bytes_read)
    }

    #[inline(always)]
    fn peek_exact_bytes(&mut self, buf: &mut [u8]) -> Result<(), ZByteIoError> {
        self.read_exact_bytes(buf)?;
        self.rewind(buf.len());
        Ok(())
    }

    #[inline(always)]
    fn z_seek(&mut self, from: ZSeekFrom) -> Result<u64, ZByteIoError> {
        match from {
            ZSeekFrom::Start(position) => {
                // set position to be seek
                // we know this can't overflow
                self.position = usize::try_from(position).map_err(ZByteIoError::from)?;
            }
            ZSeekFrom::End(position) => {
                // size of stream
                let end = self.z_size()?;
                let new_position = end + position;
                self.position = usize::try_from(new_position).map_err(ZByteIoError::from)?;
            }
            ZSeekFrom::Current(position) => {
                let current_position = i64::try_from(self.position).map_err(ZByteIoError::from)?;
                let new_position = current_position + position;
                self.position = usize::try_from(new_position).map_err(ZByteIoError::from)?;
            }
        }
        Ok(self.position as u64)
    }

    #[inline(always)]
    fn is_eof(&mut self) -> Result<bool, ZByteIoError> {
        Ok(self.position >= self.stream.get_len())
    }

    #[inline(always)]
    fn z_size(&mut self) -> Result<i64, ZByteIoError> {
        Ok(self.stream.get_len() as i64)
    }

    #[inline(always)]
    fn name(&self) -> &'static str {
        "ZByteBuffer<T>"
    }

    #[inline(always)]
    fn z_position(&mut self) -> Result<u64, ZByteIoError> {
        Ok(self.position as u64)
    }

    fn read_remaining(&mut self, sink: &mut Vec<u8>) -> Result<usize, ZByteIoError> {
        let start = self.position;
        let end = self.stream.get_len();
        match self.stream.get_slice(start..end) {
            None => {
                return Err(ZByteIoError::Generic(
                    "Somehow read remaining couldn't satisfy it's invariants"
                ))
            }
            Some(e) => {
                sink.extend_from_slice(e);
            }
        }
        Ok(end - start)
    }
}
