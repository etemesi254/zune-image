/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::path::Path;

use png::Transformations;
use zune_core::bytestream::ZCursor;

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
    read(path).unwrap()
}

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(data);
    let expand = Transformations::EXPAND;
    decoder.set_transformations(expand);

    let mut reader = decoder.read_info().unwrap();

    // Allocate the output buffer.
    let mut buf = vec![0; reader.output_buffer_size()];
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
    for ((pos, a), b) in ref_results.iter().enumerate().zip(&zune_results) {
        if a != b {
            panic!("[{pos}]: {a} != {b}");
        }
    }
    //assert_eq!(&ref_results, &zune_results);
}

#[test]
fn test_palette_1bpp() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi3p01.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn3p01.png";

        test_decoding(path);
    }
}

#[test]
fn test_palette_2bpp() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi3p02.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn3p02.png";

        test_decoding(path);
    }
}

#[test]
fn test_palette_4bpp() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi3p04.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn3p04.png";

        test_decoding(path);
    }
}

#[test]
fn test_palette_8bpp() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn3p08.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi3p08.png";

        test_decoding(path);
    }
}
