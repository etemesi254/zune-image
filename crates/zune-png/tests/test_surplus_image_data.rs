/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! IDAT streams that inflate to more bytes than the image needs.
//!
//! libpng (and the `png` crate) decode the image and ignore the surplus
//! ("IDAT: Too much image data" is only a warning).

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    let mut crc = !0_u32;
    for &byte in &out[start..] {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// zlib stream of `data` in stored deflate blocks of at most `block` bytes
fn zlib_stored(data: &[u8], block: usize) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = data.chunks(block).collect();
    for (i, part) in blocks.iter().enumerate() {
        out.push(u8::from(i + 1 == blocks.len()));
        out.extend_from_slice(&(part.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(part.len() as u16)).to_le_bytes());
        out.extend_from_slice(part);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// zlib stream of `data` and then `run` bytes of `fill`, in one fixed Huffman block: `data`
/// as literals, the run as matches of length 258 at distance 1. The inflater writes such
/// matches in large steps, so it decodes well past the limit before it stops.
fn zlib_fixed_huffman(data: &[u8], fill: u8, run: usize) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let (mut acc, mut bits) = (0_u32, 0_u32);
    let mut put = |out: &mut Vec<u8>, value: u32, len: u32| {
        acc |= value << bits;
        bits += len;
        while bits >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            bits -= 8;
        }
    };
    // Huffman codes go in most significant bit first
    let code = |code: u32, len: u32| (code.reverse_bits() >> (32 - len), len);
    let literal = |byte: u8| match byte {
        0..=143 => code(0x30 + u32::from(byte), 8),
        _ => code(0x190 + u32::from(byte) - 144, 9),
    };
    put(&mut out, 0b011, 3); // BFINAL, BTYPE 01
    for &byte in data.iter().chain(core::iter::repeat_n(&fill, run.min(1))) {
        let (value, len) = literal(byte);
        put(&mut out, value, len);
    }
    let mut left = run.saturating_sub(1);
    while left >= 258 {
        let (value, len) = code(0xC5, 8); // symbol 285: length 258
        put(&mut out, value, len);
        put(&mut out, 0, 5); // distance code 0: distance 1
        left -= 258;
    }
    for _ in 0..left {
        let (value, len) = literal(fill);
        put(&mut out, value, len);
    }
    put(&mut out, 0, 7); // end of block
    put(&mut out, 0, 7); // pad to a whole byte
    let mut all = data.to_vec();
    all.extend(core::iter::repeat_n(fill, run));
    out.extend_from_slice(&adler32(&all).to_be_bytes());
    out
}

fn image(
    width: usize, height: usize, color_type: u8, depth: u8, interlace: u8,
) -> (Vec<u8>, Vec<u8>) {
    // returns (ihdr, scanline data), every filter type None
    let channels = match color_type {
        2 => 3,
        4 => 2,
        6 => 4,
        _ => 1,
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[depth, color_type, 0, 0, interlace]);
    let mut raw = Vec::new();
    if interlace == 0 {
        let row = (width * channels * usize::from(depth)).div_ceil(8);
        for y in 0..height {
            raw.push(0);
            raw.extend((0..row).map(|x| (x * 31 + y * 17) as u8));
        }
    } else {
        const PASSES: [(usize, usize, usize, usize); 7] = [
            (0, 0, 8, 8),
            (4, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        ];
        for (xo, yo, xs, ys) in PASSES {
            let pw = width.saturating_sub(xo).div_ceil(xs);
            let ph = height.saturating_sub(yo).div_ceil(ys);
            if pw == 0 || ph == 0 {
                continue;
            }
            let row = (pw * channels * usize::from(depth)).div_ceil(8);
            for y in 0..ph {
                raw.push(0);
                raw.extend((0..row).map(|x| (x * 29 + y * 13 + xo) as u8));
            }
        }
    }
    (ihdr, raw)
}

fn build(ihdr: &[u8], plte: bool, zlib: &[u8]) -> Vec<u8> {
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut png, b"IHDR", ihdr);
    if plte {
        chunk(
            &mut png,
            b"PLTE",
            &(0..48).map(|x| (x * 5) as u8).collect::<Vec<_>>(),
        );
    }
    chunk(&mut png, b"IDAT", zlib);
    chunk(&mut png, b"IEND", &[]);
    png
}

fn reference(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut buf).unwrap();
    buf
}

fn check(width: usize, height: usize, color_type: u8, depth: u8, interlace: u8) {
    let (ihdr, raw) = image(width, height, color_type, depth, interlace);
    for surplus in [1_usize, 3, 100, 5000, 70_000] {
        let mut padded = raw.clone();
        padded.extend((0..surplus).map(|x| (x * 7 + 1) as u8));
        let streams = [
            zlib_stored(&padded, 65535),
            zlib_stored(&padded, 1000),
            zlib_fixed_huffman(&raw, 0xFF, surplus),
        ];
        for zlib in &streams {
            let png = build(&ihdr, color_type == 3, zlib);
            let expected = reference(&png);
            let got = zune_png::PngDecoder::new(ZCursor::new(&png))
                .decode_raw()
                .unwrap_or_else(|e| {
                    panic!(
                        "{width}x{height} ct{color_type} bd{depth} il{interlace} +{surplus}: {e:?}"
                    )
                });
            assert_eq!(
                got, expected,
                "{width}x{height} ct{color_type} bd{depth} il{interlace} +{surplus}"
            );
        }
    }
}

#[test]
fn one_surplus_byte() {
    // 4x3 8-bit grayscale; the zlib stream inflates to 16 bytes, the image needs 15
    // (3 rows of a filter byte and 4 pixels). libpng and the `png` crate decode it.
    const PNG: [u8; 71] = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x00, 0x00, 0x91,
        0x9F, 0xF1, 0x1A, 0x00, 0x00, 0x00, 0x0E, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x10,
        0x00, 0x02, 0x06, 0x04, 0x01, 0x00, 0x06, 0x70, 0x00, 0xC1, 0x63, 0x80, 0xFD, 0xAA, 0x00,
        0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let pixels = zune_png::PngDecoder::new(ZCursor::new(&PNG[..]))
        .decode_raw()
        .unwrap();
    assert_eq!(pixels, [0x10; 12]);
    assert_eq!(pixels, reference(&PNG));
}

#[test]
fn surplus_image_data_is_ignored() {
    check(7, 5, 0, 8, 0);
    check(20, 9, 2, 8, 0);
    check(33, 17, 6, 16, 0);
    check(33, 17, 3, 2, 0);
    check(33, 17, 0, 1, 0);
}

#[test]
fn surplus_image_data_is_ignored_interlaced() {
    check(7, 5, 0, 8, 1);
    check(20, 9, 2, 8, 1);
    check(33, 17, 3, 4, 1);
    check(1, 1, 6, 16, 1);
}

#[test]
fn surplus_image_data_is_an_error_in_strict_mode() {
    let (ihdr, mut raw) = image(7, 5, 0, 8, 0);
    raw.push(0);
    let png = build(&ihdr, false, &zlib_stored(&raw, 65535));
    let options = DecoderOptions::default().set_strict_mode(true);
    assert!(
        zune_png::PngDecoder::new_with_options(ZCursor::new(&png), options)
            .decode_raw()
            .is_err()
    );
}
