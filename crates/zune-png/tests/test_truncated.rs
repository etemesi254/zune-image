/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::io::Cursor;

use zune_core::bytestream::ZCursor;

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();

    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let _ = reader.next_frame(&mut buf).unwrap();

    buf
}

fn decode_zune(data: &[u8]) -> Vec<u8> {
    zune_png::PngDecoder::new(ZCursor::new(data))
        .decode_raw()
        .unwrap()
}

#[test]
fn test_file_ending_after_iend_header() {
    // The file stops right after the IEND chunk header, without its CRC
    for name in ["basn2c08.png", "basi2c08.png", "basn3p04.png"] {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/" + name;
        let mut contents = read(path).unwrap();
        contents.truncate(contents.len() - 4);

        assert_eq!(decode_zune(&contents), decode_ref(&contents), "{name}");
    }
}
