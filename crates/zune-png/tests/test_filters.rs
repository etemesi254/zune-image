/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::path::Path;

use zune_core::bytestream::ZCursor;

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
    read(path).unwrap()
}

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let decoder = png::Decoder::new(data);
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
    assert_eq!(&zune_results, &ref_results);
}

#[test]
fn test_none() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f00n0g08.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f00n2c08.png";

        test_decoding(path);
    }
}

#[test]
fn test_sub() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f01n0g08.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f01n2c08.png";

        test_decoding(path);
    }
}

#[test]
fn test_up() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f02n0g08.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f02n2c08.png";

        test_decoding(path);
    }
}

#[test]
fn test_avg() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f03n0g08.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f03n2c08.png";

        test_decoding(path);
    }
}

#[test]
fn test_paeth() {
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f04n0g08.png";

        test_decoding(path);
    }
    {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f04n2c08.png";

        test_decoding(path);
    }
}
