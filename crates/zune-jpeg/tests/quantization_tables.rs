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
