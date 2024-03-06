use zune_core::bytestream::{ZByteIoTrait, ZReader};
use zune_core::log::trace;
use zune_core::options::DecoderOptions;

use crate::enums::DisposalMethod;
use crate::errors::GifDecoderErrors;

#[derive(Default)]
struct DisposeArea {
    left:   usize,
    top:    usize,
    width:  usize,
    height: usize
}
pub struct GifDecoder<T: ZByteIoTrait> {
    stream:       ZReader<T>,
    options:      DecoderOptions,
    width:        usize,
    height:       usize,
    flags:        u8,
    bgindex:      u8,
    ratio:        u8,
    read_headers: bool,
    _background:  u16, // current b
    frame_pos:    usize,
    pal:          [[u8; 4]; 256],
    dispose_area: DisposeArea,
    background:   Vec<u8>
}

impl<T: ZByteIoTrait> GifDecoder<T> {
    pub fn new(source: T) -> GifDecoder<T> {
        return GifDecoder::new_with_options(source, DecoderOptions::new_fast());
    }
    pub fn new_with_options(source: T, options: DecoderOptions) -> GifDecoder<T> {
        return GifDecoder {
            stream: ZReader::new(source),
            options,
            ..Default::default()
        };
    }
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
            self.parse_colortable(2 << (self.flags & 0b111), usize::MAX)?;
        }
        trace!("Image width  :{}", self.width);
        trace!("Image height :{}", self.height);
        trace!("Ratio: {}", self.ratio);
        self.read_headers = true;

        Ok(())
    }
    fn parse_colortable(&mut self, num_entries: usize, transp: usize) -> Result<(), &'static str> {
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

    pub fn output_buf_size(&self) -> Option<usize> {
        if self.read_headers {
            return self.width.checked_mul(self.height)?.checked_mul(4);
        }
        None
    }

    #[inline]
    fn fill_rect(dispose_area: &DisposeArea, width: usize, output: &mut [u8], color: u32) {
        output
            .chunks_exact_mut(width)
            .skip(dispose_area.top)
            .take(dispose_area.height)
            .for_each(|x| {
                x.chunks_exact_mut(4)
                    .skip(dispose_area.left)
                    .take(dispose_area.width)
                    .for_each(|x| x.copy_from_slice(&color.to_le_bytes()))
            });
    }
    pub fn decode_into(
        &mut self, output: &mut [u8], two_back: Option<&[u8]>
    ) -> Result<(), GifDecoderErrors> {
        self.decode_headers()?;

        let output_size = self
            .output_buf_size()
            .ok_or(GifDecoderErrors::OverflowError(
                "cannot calculate output dimensions"
            ))?;

        if output_size > output.len() {
            return Err(GifDecoderErrors::TooSmallSize(output_size, output.len()));
        }
        let output = &mut output[..output_size];

        if self.frame_pos == 0 {
            // zero out everything
            self.background.resize(output_size, 0);
            output.fill(0);
        } else {
            // second frame, figure out how to dispose the first one

            let mut dispose = DisposalMethod::from_flags((self.flags & 0x1C) >> 2);
            let pix_count = self.width * self.height;

            if dispose == DisposalMethod::Restore && two_back.is_none() {
                // fall back to background if we lack a background from two frames back
                dispose = DisposalMethod::Background;
            }
            match dispose {
                DisposalMethod::None | DisposalMethod::InPlace => {
                    // ignore
                }
                DisposalMethod::Background => {
                    // use previous
                    // Self::fill_rect(&self.dispose_area, self.width, output);
                }
                DisposalMethod::Restore => {}
            }
        }
        self.frame_pos += 1;

        Ok(())
    }
}

fn test_gif<T: ZByteIoTrait>(buffer: &mut ZReader<T>) -> bool {
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
