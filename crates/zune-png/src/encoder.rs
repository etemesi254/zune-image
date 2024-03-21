/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::vec::Vec;

use zune_core::bytestream::{ZByteIoError, ZByteWriter, ZByteWriterTrait};
use zune_core::options::EncoderOptions;
use zune_inflate::DeflateEncoder;

use crate::constants::PNG_SIGNATURE;
use crate::decoder::PngChunk;
use crate::enums::{FilterMethod, PngChunkType};
use crate::filters::{choose_compression_filter, filter_scanline};
use crate::headers::writers::{
    write_chunk, write_exif, write_gamma, write_header_fn, write_iend, write_ihdr
};

#[derive(Default)]
pub struct PngEncoder<'a> {
    pub(crate) options:         EncoderOptions,
    pub(crate) data:            &'a [u8],
    pub(crate) row_filter:      FilterMethod,
    pub(crate) encoded_chunks:  Vec<u8>,
    pub(crate) filter_scanline: Vec<u8>,
    pub(crate) gamma:           Option<f32>,
    pub(crate) exif:            Option<&'a [u8]>
}

impl<'a> PngEncoder<'a> {
    /// Create a new encoder that can encode an image into a PNG chunk
    ///
    /// # Endianness
    ///
    /// If you are encoding 16 bit data, it is expected that
    /// the data is laid  out in big endian (in order to avoid a
    /// potentially expensive clone and conversion step)
    pub fn new(data: &'a [u8], options: EncoderOptions) -> PngEncoder<'a> {
        PngEncoder {
            options,
            data,
            row_filter: FilterMethod::None,
            ..Default::default()
        }
    }

    /// Add exif data which will be encoded
    pub fn add_exif_segment(&mut self, exif: &'a [u8]) {
        self.exif = Some(exif);
    }

    pub fn encode_headers<T: ZByteWriterTrait>(
        &self, writer: &mut ZByteWriter<T>
    ) -> Result<(), ZByteIoError> {
        // write signature
        writer.write_u64_be(PNG_SIGNATURE);
        // write ihdr
        write_header_fn(self, writer, b"IHDR", write_ihdr)?;

        // extra headers
        // need to check their existence because  write_header_fn will do
        // some writing even if they don't exist
        if self.exif.is_some() {
            write_header_fn(self, writer, b"eXIf", write_exif)?;
        }
        if self.gamma.is_some() {
            write_header_fn(self, writer, b"gAMA", write_gamma)?;
        }
        Ok(())
    }

    pub fn encode<T: ZByteWriterTrait>(&mut self, sink: T) -> Result<usize, ZByteIoError> {
        let mut writer = ZByteWriter::new(sink);

        self.encode_headers(&mut writer)?;

        // encode filters
        self.add_filters();

        self.write_idat_chunks(&mut writer)?;

        write_header_fn(self, &mut writer, b"IEND", write_iend)?;

        // let position = writer.position();
        // out_size.truncate(position);

        Ok(writer.bytes_written())
    }

    const fn calculate_scanline_size(&self) -> usize {
        self.options.width()
            * self.options.depth().size_of()
            * self.options.colorspace().num_components()
    }

    fn add_filters(&mut self) {
        let scanline_length = (self.calculate_scanline_size() + 1)
            .checked_mul(self.options.height())
            .unwrap();
        let components =
            self.options.colorspace().num_components() * self.options.depth().size_of();

        // allocate space for filtered scanline
        self.filter_scanline.resize(scanline_length, 0);

        // one row above the current processing row
        let mut previous_scanline: &[u8] = &[];

        let scanline_size = self.calculate_scanline_size();

        for (i, filter_s) in self
            .filter_scanline
            .chunks_exact_mut(scanline_size + 1)
            .take(self.options.height())
            .enumerate()
        {
            let (previous, current) = self.data.split_at(i * scanline_size);

            if i > 0 {
                // previous row now becomes defined
                previous_scanline = &previous[(i - 1) * scanline_size..];
            }
            let current_scanline = &current[0..scanline_size];
            let filter = choose_compression_filter(previous_scanline, current_scanline);

            filter_scanline(
                current_scanline,
                previous_scanline,
                filter_s,
                filter,
                components
            );
        }
        // encode filtered scanline
        self.encoded_chunks = DeflateEncoder::new(&self.filter_scanline).encode_zlib();
    }
    fn write_idat_chunks<T: ZByteWriterTrait>(
        &self, writer: &mut ZByteWriter<T>
    ) -> Result<(), ZByteIoError> {
        debug_assert!(!self.encoded_chunks.is_empty());
        // Most decoders love data in 8KB chunks, since
        // probably libpng does that by default
        // so let's try emulating that
        for chunk in self.encoded_chunks.chunks(8192) {
            let chunk_type = PngChunk {
                length:     chunk.len(),
                chunk_type: PngChunkType::IDAT, // not needed
                chunk:      *b"IDAT",
                crc:        0 // not needed
            };
            write_chunk(chunk_type, chunk, writer)?;
        }
        Ok(())
    }
}

#[test]
fn test_simple_write() {
    use zune_core::bit_depth::BitDepth;
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;

    use crate::PngDecoder;

    let width = 40;
    let height = 10;
    let data = vec![100; width * height];

    let options = EncoderOptions::default()
        .set_colorspace(ColorSpace::Luma)
        .set_width(40)
        .set_height(10)
        .set_depth(BitDepth::Eight);

    let mut encoder = PngEncoder::new(&data, options);
    let mut sink = vec![];

    let _ = encoder.encode(&mut sink).unwrap();
    let mut hello = PngDecoder::new(ZCursor::new(&sink));
    let bytes = hello.decode_raw().unwrap();
    assert_eq!(&data, &bytes);
}
