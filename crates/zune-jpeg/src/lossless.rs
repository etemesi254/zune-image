/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Predictive Huffman decoding for 8-bit, single-component SOF3 frames.

use alloc::string::ToString;
use alloc::vec;

use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::log::warn;

use crate::decoder::JpegDecoder;
use crate::errors::{DecodeErrors, UnsupportedSchemes};
use crate::huffman::HuffmanTable;
use crate::marker::Marker;

// Unlike the DCT lookahead reader, this reader never synthesizes entropy bits.
struct LosslessBits {
    byte:      u8,
    remaining: u8
}

impl LosslessBits {
    fn bit<T: ZByteReaderTrait>(&mut self, reader: &mut ZReader<T>) -> Result<i32, DecodeErrors> {
        if self.remaining == 0 {
            self.byte = reader.read_u8_err()?;
            if self.byte == 0xff && reader.read_u8_err()? != 0 {
                return Err(DecodeErrors::FormatStatic(
                    "Premature marker in lossless JPEG scan"
                ));
            }
            self.remaining = 8;
        }
        self.remaining -= 1;
        Ok(i32::from((self.byte >> self.remaining) & 1))
    }

    fn difference<T: ZByteReaderTrait>(
        &mut self, reader: &mut ZReader<T>, table: &HuffmanTable
    ) -> Result<i32, DecodeErrors> {
        let mut code = 0;
        for length in 1..=16 {
            code = (code << 1) | self.bit(reader)?;
            if table.maxcode[length] >= 0 && (code << (16 - length)) < table.maxcode[length] {
                let index = usize::try_from(code + table.offset[length])
                    .map_err(|_| DecodeErrors::FormatStatic("Invalid lossless Huffman code"))?;
                let category = *table
                    .values
                    .get(index)
                    .ok_or(DecodeErrors::FormatStatic("Invalid lossless Huffman code"))?;
                if category == 16 {
                    return Ok(32768);
                }
                let mut value = 0;
                for _ in 0..category {
                    value = (value << 1) | self.bit(reader)?;
                }
                if category != 0 && value < (1 << (category - 1)) {
                    value -= (1 << category) - 1;
                }
                return Ok(value);
            }
        }
        Err(DecodeErrors::HuffmanDecode(
            "Invalid lossless Huffman code".to_string()
        ))
    }

    fn marker<T: ZByteReaderTrait>(
        &mut self, reader: &mut ZReader<T>
    ) -> Result<Marker, DecodeErrors> {
        self.remaining = 0;
        if reader.read_u8_err()? != 0xff {
            return Err(DecodeErrors::FormatStatic(
                "Expected marker after lossless JPEG entropy"
            ));
        }
        let mut byte = reader.read_u8_err()?;
        while byte == 0xff {
            byte = reader.read_u8_err()?;
        }
        Marker::from_u8(byte)
            .filter(|marker| !matches!(marker, Marker::UNKNOWN(_)))
            .ok_or(DecodeErrors::FormatStatic(
                "Invalid marker after lossless JPEG entropy"
            ))
    }
}

impl<T: ZByteReaderTrait> JpegDecoder<T> {
    pub(crate) fn validate_lossless_encoding(&self) -> Result<(), DecodeErrors> {
        if self.info.pixel_density != 8 || self.components.len() != 1 || self.num_scans != 1 {
            return Err(DecodeErrors::Unsupported(
                UnsupportedSchemes::LosslessHuffman
            ));
        }
        if self.info.height == 0 || self.dnl_image {
            return Err(DecodeErrors::FormatStatic(
                "Lossless JPEG with DNL is not supported"
            ));
        }
        let component = &self.components[0];
        if component.quantization_table_number != 0 || component.ac_huff_table != 0 {
            return Err(DecodeErrors::FormatStatic(
                "Lossless JPEG requires zero quantization and AC table selectors"
            ));
        }
        if self.restart_interval % usize::from(self.info.width) != 0 {
            return Err(DecodeErrors::FormatStatic(
                "Lossless JPEG restart interval must contain whole rows"
            ));
        }
        if !matches!(
            self.options.jpeg_get_out_colorspace(),
            ColorSpace::Luma
                | ColorSpace::LumaA
                | ColorSpace::RGB
                | ColorSpace::BGR
                | ColorSpace::RGBA
                | ColorSpace::BGRA
        ) {
            return Err(DecodeErrors::FormatStatic(
                "Unsupported lossless JPEG output colorspace"
            ));
        }
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) fn decode_lossless(&mut self, output: &mut [u8]) -> Result<(), DecodeErrors> {
        let width = usize::from(self.info.width);
        let height = usize::from(self.info.height);
        let channels = self.options.jpeg_get_out_colorspace().num_components();
        let table_index = self.components[0].dc_huff_table;
        let table = self
            .entropy_tables
            .dc_huffman
            .get(table_index)
            .and_then(Option::as_ref)
            .ok_or(DecodeErrors::FormatStatic(
                "Missing lossless JPEG Huffman table"
            ))?;
        let mut previous = vec![0u16; width];
        let mut current = vec![0u16; width];
        let mut bits = LosslessBits {
            byte:      0,
            remaining: 0
        };
        let mut next_restart = 0;
        let mut interval_start = 0;
        let initial = 1 << (7 - self.succ_low);
        let mut cancel = self.cancel_debounced(width);
        for y in 0..height {
            if cancel.is_cancelled() {
                return Err(DecodeErrors::Cancelled);
            }
            let row_start = y * width;
            if self.restart_interval != 0
                && row_start != 0
                && row_start % self.restart_interval == 0
            {
                if bits.marker(&mut self.stream)? != Marker::RST(next_restart) {
                    return Err(DecodeErrors::FormatStatic(
                        "Incorrect lossless JPEG restart sequence"
                    ));
                }
                next_restart = (next_restart + 1) & 7;
                interval_start = y;
            }
            for x in 0..width {
                let prediction = if y == interval_start {
                    if x == 0 {
                        initial
                    } else {
                        i32::from(current[x - 1])
                    }
                } else if x == 0 {
                    i32::from(previous[x])
                } else {
                    let a = i32::from(current[x - 1]);
                    let b = i32::from(previous[x]);
                    let c = i32::from(previous[x - 1]);
                    match self.spec_start {
                        1 => a,
                        2 => b,
                        3 => c,
                        4 => a + b - c,
                        5 => a + ((b - c) >> 1),
                        6 => b + ((a - c) >> 1),
                        7 => (a + b) >> 1,
                        _ => {
                            return Err(DecodeErrors::FormatStatic(
                                "Invalid lossless JPEG predictor"
                            ))
                        }
                    }
                };
                let difference = bits.difference(&mut self.stream, table)?;
                // T.81 H.2 reconstructs modulo 2^16, before the point transform.
                current[x] = ((prediction + difference) & 0xffff) as u16;
                let sample = (current[x] << self.succ_low) as u8;
                let at = (row_start + x) * channels;
                let pixel = &mut output[at..at + channels];
                pixel.fill(sample);
                if matches!(
                    self.options.jpeg_get_out_colorspace(),
                    ColorSpace::LumaA | ColorSpace::RGBA | ColorSpace::BGRA
                ) {
                    pixel[channels - 1] = 255;
                }
            }
            self.pixels_decoded = (row_start + width) * channels;
            core::mem::swap(&mut previous, &mut current);
        }
        loop {
            match bits.marker(&mut self.stream) {
                Ok(Marker::EOI) => return Ok(()),
                Ok(marker @ (Marker::APP(_) | Marker::COM)) => self.parse_marker_inner(marker)?,
                Ok(_) => {
                    return Err(DecodeErrors::FormatStatic(
                        "Unexpected marker after lossless JPEG scan"
                    ))
                }
                Err(error) if error.is_recoverable_eof() && !self.scan_eof_is_error() => {
                    warn!(
                        "Lossless JPEG samples are complete but the end marker is missing: {error}"
                    );
                    return Ok(());
                }
                Err(error) => return Err(error)
            }
        }
    }
}
