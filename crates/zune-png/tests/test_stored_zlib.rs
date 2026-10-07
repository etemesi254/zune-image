/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bytestream::ZCursor;

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

/// zlib stream of `data` in stored (uncompressed) deflate blocks of at most `block` bytes
fn zlib_stored(data: &[u8], block: usize) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = data.chunks(block).collect();
    for (i, part) in blocks.iter().enumerate() {
        out.push(u8::from(i + 1 == blocks.len())); // BFINAL, BTYPE 00
        out.extend_from_slice(&(part.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(part.len() as u16)).to_le_bytes());
        out.extend_from_slice(part);
    }
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

#[test]
fn image_data_in_several_stored_deflate_blocks() {
    // A 128x128 RGB image in stored deflate blocks (as zlib level 0 writes it). Decoding
    // went wrong from the second block on: wrong pixels, or "Reserved block type".
    let (width, height) = (128_usize, 128_usize);
    let mut raw = Vec::new();
    for y in 0..height {
        raw.push(0); // filter type None
        raw.extend((0..width * 3).map(|x| ((x * 31 + y * 17) ^ (x * y)) as u8));
    }

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &zlib_stored(&raw, 31745));
    chunk(&mut png, b"IEND", &[]);

    let pixels = zune_png::PngDecoder::new(ZCursor::new(&png))
        .decode_raw()
        .unwrap();
    let expected: Vec<u8> = raw
        .chunks_exact(1 + width * 3)
        .flat_map(|row| row[1..].iter().copied())
        .collect();
    assert!(pixels == expected);
}
