/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for the per-component quantization tables of the raw output session.
//!
//! The expected tables are read straight from the file's DQT and SOF markers
//! by a small walker here, so the test does not share code with the decoder.

use std::cell::Cell;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::rc::Rc;

use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

/// Zig-zag index -> natural (row-major) index.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63
];

/// Walk the markers up to the first SOS and return, per SOF component, the
/// natural-order table it selects.
fn tables_in_file(bytes: &[u8]) -> Vec<[u16; 64]> {
    let mut defined: [Option<[u16; 64]>; 4] = [None; 4];
    let mut selected = Vec::new();
    let mut pos = 2;
    while pos + 4 <= bytes.len() {
        assert_eq!(bytes[pos], 0xFF);
        let marker = bytes[pos + 1];
        if marker == 0xFF {
            pos += 1;
            continue;
        }
        let length = usize::from(u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]));
        let body = &bytes[pos + 4..pos + 2 + length];
        match marker {
            0xDB => {
                let mut at = 0;
                while at < body.len() {
                    let precision = body[at] >> 4;
                    let position = usize::from(body[at] & 0x0F);
                    at += 1;
                    let mut table = [0u16; 64];
                    for (i, natural) in ZIGZAG.iter().enumerate() {
                        table[*natural] = if precision == 0 {
                            u16::from(body[at + i])
                        } else {
                            u16::from_be_bytes([body[at + 2 * i], body[at + 2 * i + 1]])
                        };
                    }
                    at += 64 * (usize::from(precision) + 1);
                    defined[position] = Some(table);
                }
            }
            0xC0..=0xC2 => {
                let count = usize::from(body[5]);
                for c in 0..count {
                    selected.push(usize::from(body[6 + 3 * c + 2]));
                }
            }
            0xDA => break,
            _ => {}
        }
        pos += 2 + length;
    }
    selected
        .into_iter()
        .map(|position| defined[position].expect("table defined before SOS"))
        .collect()
}

fn tables_from_decoder(bytes: &[u8]) -> Vec<[u16; 64]> {
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().expect("decode_headers failed");
    let raw = decoder.raw_output();
    raw.quantization_tables().expect("quantization tables")
}

#[test]
fn quantization_tables_unavailable_before_headers() {
    let mut decoder = JpegDecoder::new(ZCursor::new(&[][..]));
    let raw = decoder.raw_output();
    assert!(raw.quantization_tables().is_none());
}

#[test]
fn quantization_tables_444() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let tables = tables_from_decoder(bytes);
    assert_eq!(tables.len(), 3);
    assert_eq!(tables, tables_in_file(bytes));
}

#[test]
fn quantization_tables_420() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let tables = tables_from_decoder(bytes);
    assert_eq!(tables.len(), 3);
    assert_eq!(tables, tables_in_file(bytes));
    // Luma and chroma usually select different tables.
    assert_ne!(tables[0], tables[1]);
}

#[test]
fn quantization_tables_are_in_natural_order() {
    let bytes = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let tables = tables_from_decoder(bytes);
    assert_eq!(tables, tables_in_file(bytes));
}

/// Offsets of the file's SOS markers.
fn sos_offsets(bytes: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut pos = 2;
    while pos + 4 <= bytes.len() {
        let marker = bytes[pos + 1];
        if bytes[pos] != 0xFF || marker == 0xFF || marker == 0x00 || (0xD0..=0xD7).contains(&marker)
        {
            pos += 1;
            continue;
        }
        if marker == 0xD9 {
            break;
        }
        let length = usize::from(u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]));
        if marker == 0xDA {
            offsets.push(pos);
        }
        pos += 2 + length;
    }
    offsets
}

/// The file with a DQT redefining table 0 inserted before its second SOS.
fn with_dqt_between_scans(bytes: &[u8]) -> Vec<u8> {
    let sos = sos_offsets(bytes);
    assert!(sos.len() >= 2, "expected more than one scan");
    let mut out = bytes[..sos[1]].to_vec();
    out.extend([0xFF, 0xDB, 0x00, 0x43, 0x00]);
    out.extend([99u8; 64]);
    out.extend(&bytes[sos[1]..]);
    out
}

/// A reader that exposes `limit` bytes, so a decode can run out of data and be retried.
struct Growing<'a> {
    data:     &'a [u8],
    position: usize,
    limit:    Rc<Cell<usize>>
}

impl Growing<'_> {
    fn visible(&self) -> &[u8] {
        &self.data[..self.limit.get().min(self.data.len())]
    }
}

impl Read for Growing<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.fill_buf()?.read(buf)?;
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for Growing<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let visible = self.visible();
        Ok(visible.get(self.position..).unwrap_or(&[]))
    }

    fn consume(&mut self, amt: usize) {
        self.position += amt;
    }
}

impl Seek for Growing<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let position = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::Current(p) => self.position as i64 + p,
            SeekFrom::End(p) => self.visible().len() as i64 + p
        };
        self.position = usize::try_from(position)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        Ok(self.position as u64)
    }
}

fn check_dqt_between_scans(original: &[u8]) {
    let expected = tables_in_file(original);
    let bytes = with_dqt_between_scans(original);

    let mut decoder = JpegDecoder::new(ZCursor::new(&bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    assert_eq!(raw.quantization_tables().unwrap(), expected);
    let layout = raw.layout().unwrap();
    let components = raw.num_components().unwrap();
    let mut planes: Vec<Vec<u8>> = layout[..components]
        .iter()
        .map(|l| vec![0; l.byte_size])
        .collect();
    let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes(&mut refs).unwrap();
    assert_eq!(raw.quantization_tables().unwrap(), expected);

    let limit = Rc::new(Cell::new(0));
    let mut decoder = JpegDecoder::new(Growing {
        data:     &bytes,
        position: 0,
        limit:    Rc::clone(&limit)
    });
    loop {
        limit.set(limit.get() + 97);
        match decoder.decode_headers() {
            Ok(()) => break,
            Err(e) if e.is_recoverable_eof() => {}
            Err(e) => panic!("{e:?}")
        }
    }
    let mut raw = decoder.raw_output();
    let mut retried: Vec<Vec<u8>> = layout[..components]
        .iter()
        .map(|l| vec![0; l.byte_size])
        .collect();
    loop {
        let mut refs: Vec<&mut [u8]> = retried.iter_mut().map(Vec::as_mut_slice).collect();
        match raw.decode_into_planes(&mut refs) {
            Ok(()) => break,
            Err(e) if e.is_recoverable_eof() => {
                assert_eq!(raw.quantization_tables().unwrap(), expected);
                limit.set(limit.get() + 97);
            }
            Err(e) => panic!("{e:?}")
        }
    }
    assert_eq!(raw.quantization_tables().unwrap(), expected);
    assert_eq!(retried, planes);
}

#[test]
fn quantization_tables_ignore_dqt_between_progressive_scans() {
    check_dqt_between_scans(include_bytes!(
        "../../../test-images/jpeg/progressive_restart_420.jpg"
    ));
    check_dqt_between_scans(include_bytes!(
        "../../../test-images/jpeg/progressive_440_65x65.jpg"
    ));
}

#[test]
fn quantization_tables_ignore_dqt_between_baseline_scans() {
    check_dqt_between_scans(include_bytes!(
        "../../../test-images/jpeg/non_interleaved_420_64x64.jpg"
    ));
}
