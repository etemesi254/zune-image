/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! `DecoderOptions::inflate_set_limit` also bounds the zlib data of iCCP and zTXt chunks.

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_png::PngDecoder;

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

/// zlib stream of one fixed-Huffman block: a zero byte, then `matches` copies of
/// (length 258, distance 1), so it inflates to `1 + 258 * matches` zero bytes.
fn zlib_zeros(matches: usize) -> Vec<u8> {
    let mut bits: Vec<u8> = Vec::new();
    // Huffman codes are written most significant bit first, extra bits least significant first
    let mut code = |value: u32, len: u32| {
        for i in (0..len).rev() {
            bits.push(((value >> i) & 1) as u8);
        }
    };
    code(0b110, 3); // BFINAL 1, then BTYPE 01 (fixed Huffman) least significant bit first
    code(0b0011_0000, 8); // literal 0
    for _ in 0..matches {
        code(0b1100_0101, 8); // length symbol 285 = length 258
        code(0, 5); // distance code 0 = distance 1
    }
    code(0, 7); // end of block (256)

    let mut out = vec![0x78, 0x01];
    for byte in bits.chunks(8) {
        out.push(
            byte.iter()
                .enumerate()
                .fold(0, |acc, (i, &b)| acc | (b << i))
        );
    }
    // Adler-32 of the zero bytes
    let n = (1 + 258 * matches) as u32;
    let (a, b) = (1_u32, n % 65521);
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

fn png_with(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut png, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]);
    chunk(&mut png, kind, body);
    // 1x1 gray image, filter byte 0 and one zero sample, in a stored block
    chunk(
        &mut png,
        b"IDAT",
        &[0x78, 0x01, 1, 2, 0, 0xFD, 0xFF, 0, 0, 0, 2, 0, 1]
    );
    chunk(&mut png, b"IEND", &[]);
    png
}

#[test]
fn ztxt_and_iccp_respect_the_inflate_limit() {
    // 4,000 matches: about 1 MB of text from a 2 KB chunk
    let data = zlib_zeros(4_000);
    let size = 1 + 258 * 4_000;

    let mut ztxt = b"Comment\0\0".to_vec();
    ztxt.extend_from_slice(&data);
    let mut iccp = b"icc\0\0".to_vec();
    iccp.extend_from_slice(&data);

    // without a limit both chunks inflate in full
    let png = png_with(b"zTXt", &ztxt);
    let mut decoder = PngDecoder::new(ZCursor::new(&png));
    decoder.decode_headers().unwrap();
    assert_eq!(decoder.info().unwrap().ztxt_chunk[0].text.len(), size);

    let png = png_with(b"iCCP", &iccp);
    let mut decoder = PngDecoder::new(ZCursor::new(&png));
    decoder.decode_headers().unwrap();
    assert_eq!(
        decoder.info().unwrap().icc_profile.as_ref().unwrap().len(),
        size
    );

    // with a 64 KiB limit neither is inflated, and the image still decodes
    let options = DecoderOptions::default().inflate_set_limit(1 << 16);

    let png = png_with(b"zTXt", &ztxt);
    let mut decoder = PngDecoder::new_with_options(ZCursor::new(&png), options);
    decoder.decode_headers().unwrap();
    assert!(
        decoder.info().unwrap().ztxt_chunk.is_empty(),
        "zTXt inflated past the limit"
    );
    assert_eq!(decoder.decode_raw().unwrap(), [0]);

    let png = png_with(b"iCCP", &iccp);
    let mut decoder = PngDecoder::new_with_options(ZCursor::new(&png), options);
    decoder.decode_headers().unwrap();
    assert!(
        decoder.info().unwrap().icc_profile.is_none(),
        "iCCP inflated past the limit"
    );
    assert_eq!(decoder.decode_raw().unwrap(), [0]);
}
