/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::io::Cursor;
use std::path::Path;

use png::Transformations;
use zune_core::bytestream::ZCursor;

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
    read(path).unwrap()
}

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    let expand = Transformations::EXPAND;
    decoder.set_transformations(expand);

    let mut reader = decoder.read_info().unwrap();

    // Allocate the output buffer.
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    // Read the next frame. An APNG might contain multiple frames.
    let _ = reader.next_frame(&mut buf).unwrap();

    buf
}

fn decode_zune(data: &[u8]) -> Vec<u8> {
    zune_png::PngDecoder::new(ZCursor::new(data))
        .decode_raw()
        .unwrap()
}

fn test_decoding<P: AsRef<Path>>(path: P) {
    let contents = open_and_read(path);

    let zune_results = decode_zune(&contents);
    let ref_results = decode_ref(&contents);
    assert_eq!(&zune_results, &ref_results);
}

#[test]
fn test_palette_trns_16bit() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/tbwn0g16.png";

    test_decoding(path);
}

#[test]
fn test_palette_trns_8bit() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/tbgn3p08.png";

    test_decoding(path);
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { 0xedb8_8320 ^ (crc >> 1) } else { crc >> 1 };
        }
    }
    !crc
}

#[test]
fn test_trns_longer_than_needed() {
    // A grayscale or RGB tRNS chunk with bytes after the 2 or 6 it uses: the rest of
    // the chunk must be skipped, not read as the next chunk
    for (color, trns) in [
        (png::ColorType::Grayscale, &[0, 0x40][..]),
        (png::ColorType::Rgb, &[0, 0x40, 0, 0x50, 0, 0x60][..])
    ] {
        let mut contents = Vec::new();
        let mut encoder = png::Encoder::new(&mut contents, 4, 2);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_trns(trns);
        let mut writer = encoder.write_header().unwrap();
        let samples = if color == png::ColorType::Rgb { 3 } else { 1 };
        let data: Vec<u8> = (0..8 * samples)
            .map(|i| (i as u8).wrapping_mul(16))
            .collect();
        writer.write_image_data(&data).unwrap();
        writer.finish().unwrap();

        // append 4 bytes to the tRNS chunk body and fix its length and CRC
        let at = contents.windows(4).position(|w| w == b"tRNS").unwrap() - 4;
        let len = u32::from_be_bytes(contents[at..at + 4].try_into().unwrap()) as usize;
        let body_end = at + 8 + len;
        contents.splice(body_end..body_end + 4, [1, 2, 3, 4]);
        contents[at..at + 4].copy_from_slice(&(len as u32 + 4).to_be_bytes());
        let crc = crc32(&contents[at + 4..body_end + 4]);
        contents.splice(body_end + 4..body_end + 4, crc.to_be_bytes());

        assert_eq!(decode_zune(&contents), decode_ref(&contents), "{color:?}");
    }
}
