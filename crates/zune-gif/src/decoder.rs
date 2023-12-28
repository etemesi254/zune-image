use zune_core::bytestream::{ZByteReader, ZReaderTrait};
use zune_core::log::trace;
use zune_core::options::DecoderOptions;

use crate::errors::GifDecoderErrors;

pub struct GifDecoder<T: ZReaderTrait> {
    stream:       ZByteReader<T>,
    options:      DecoderOptions,
    width:        usize,
    height:       usize,
    flags:        u8,
    bgindex:      u8,
    ratio:        u8,
    read_headers: bool,
    _background:  u16, // current b
    pal:          [[u8; 4]; 256]
}

impl<T: ZReaderTrait> GifDecoder<T> {
    pub fn decode_headers(&mut self) -> Result<(), GifDecoderErrors> {
        if self.read_headers {
            return Ok(());
        }
        if !test_gif(&mut self.stream) {
            return Err(GifDecoderErrors::NotAGif);
        }

        self.width = usize::from(self.stream.get_u16_le_err()?);
        self.height = usize::from(self.stream.get_u16_le_err()?);

        self.flags = self.stream.get_u8_err()?;
        self.bgindex = self.stream.get_u8_err()?;
        self.ratio = self.stream.get_u8_err()?;

        if self.width > self.options.get_max_width() {
            return Err(GifDecoderErrors::TooLargeDimensions(
                "width",
                self.options.get_max_width(),
                self.width
            ));
        }
        if self.height > self.options.get_max_height() {
            return Err(GifDecoderErrors::TooLargeDimensions(
                "height",
                self.options.get_max_height(),
                self.height
            ));
        }
        // check if we have a global palette
        if (self.flags & 0x80) > 0 {
            self.parse_colortable(2 << (self.flags & 7), usize::MAX)?;
        }
        trace!("Image width  :{}", self.width);
        trace!("Image height :{}", self.height);
        trace!("Ratio: {}", self.ratio);
        self.read_headers = true;

        Ok(())
    }
    fn parse_colortable(&mut self, num_entries: usize, transp: usize) -> Result<(), &'static str> {
        if !self.stream.has(num_entries * 3) {
            return Err("Not enough bytes for palette");
        }
        self.pal
            .iter_mut()
            .take(num_entries)
            .enumerate()
            .for_each(|(pos, x)| {
                // weird order
                x[2] = self.stream.get_u8();
                x[1] = self.stream.get_u8();
                x[0] = self.stream.get_u8();
                x[3] = if transp == pos { 0 } else { 255 }
            });
        Ok(())
    }
}

fn test_gif<T: ZReaderTrait>(buffer: &mut ZByteReader<T>) -> bool {
    if buffer.get_u8() != b'G'
        || buffer.get_u8() != b'I'
        || buffer.get_u8() != b'F'
        || buffer.get_u8() != b'8'
    {
        return false;
    }
    let sz = buffer.get_u8();
    if sz != b'9' && sz != b'7' {
        return false;
    }
    if buffer.get_u8() != b'a' {
        return false;
    }
    true
}
