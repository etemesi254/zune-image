use std::cell::Cell;
use std::io::{self, BufRead, Read, Seek, SeekFrom};
use std::rc::Rc;

pub struct Growing<'a> {
    bytes:    &'a [u8],
    visible:  Rc<Cell<usize>>,
    position: usize
}

impl<'a> Growing<'a> {
    pub fn new(bytes: &'a [u8], visible: Rc<Cell<usize>>) -> Self {
        Self {
            bytes,
            visible,
            position: 0
        }
    }
}

impl Read for Growing<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let input = self.fill_buf()?;
        let count = input.len().min(output.len());
        output[..count].copy_from_slice(&input[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for Growing<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let end = self.visible.get().min(self.bytes.len());
        Ok(&self.bytes[self.position.min(end)..end])
    }

    fn consume(&mut self, count: usize) {
        self.position += count;
    }
}

impl Seek for Growing<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let next = match from {
            SeekFrom::Start(value) => i64::try_from(value).ok(),
            SeekFrom::Current(value) => i64::try_from(self.position)
                .ok()
                .and_then(|position| position.checked_add(value)),
            SeekFrom::End(value) => i64::try_from(self.visible.get())
                .ok()
                .and_then(|position| position.checked_add(value))
        }
        .and_then(|position| usize::try_from(position).ok())
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        self.position = next;
        Ok(next as u64)
    }
}
