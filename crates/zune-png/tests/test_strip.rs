/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

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
use zune_core::options::DecoderOptions;

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
    read(path).unwrap()
}

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(data);
    let expand = Transformations::EXPAND | Transformations::STRIP_16;
    decoder.set_transformations(expand);

    let mut reader = decoder.read_info().unwrap();

    // Allocate the output buffer.
    let mut buf = vec![0; reader.output_buffer_size()];
    // Read the next frame. An APNG might contain multiple frames.
    let _ = reader.next_frame(&mut buf).unwrap();

    buf
}

fn decode_raw_zune(data: &[u8]) -> Vec<u8> {
    let options = DecoderOptions::default().png_set_strip_to_8bit(true);
    zune_png::PngDecoder::new_with_options(ZCursor::new(data), options)
        .decode_raw()
        .unwrap()
}

fn decode_zune(data: &[u8]) -> Vec<u8> {
    let options = DecoderOptions::default().png_set_strip_to_8bit(true);
    zune_png::PngDecoder::new_with_options(ZCursor::new(data), options)
        .decode()
        .unwrap()
        .u8()
        .unwrap()
}

fn decode_into_zune(data: &[u8]) -> Vec<u8> {
    let options = DecoderOptions::default().png_set_strip_to_8bit(true);
    let mut decoder = zune_png::PngDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    let size = decoder.output_buffer_size().unwrap();
    let mut buffer = vec![0; size];
    decoder.decode_into(&mut buffer).unwrap();
    buffer
}

fn test_decoding<P: AsRef<Path>>(path: P) {
    let contents = open_and_read(path);

    let zune_results = decode_raw_zune(&contents);
    let ref_results = decode_ref(&contents);
    assert_eq!(&zune_results, &ref_results);
}

fn test_enum_decoding<P: AsRef<Path>>(path: P) {
    let contents = open_and_read(path);

    let zune_results = decode_zune(&contents);
    let ref_results = decode_ref(&contents);
    assert_eq!(&zune_results, &ref_results);
}

fn test_into_decoding<P: AsRef<Path>>(path: P) {
    let contents = open_and_read(path);

    let zune_results = decode_into_zune(&contents);
    let ref_results = decode_ref(&contents);
    assert_eq!(&zune_results, &ref_results);
}

#[test]
fn test_strip_16bit_basic() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn6a16.png";

    test_decoding(&path);
    test_into_decoding(&path);
    test_enum_decoding(&path);
}

#[test]
fn test_strip_16bit_1() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn4a16.png";
    test_decoding(&path);
    test_into_decoding(&path);
    test_enum_decoding(&path);
}

#[test]
fn test_strip_16bit_2() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi2c16.png";
    test_decoding(&path);
    test_into_decoding(&path);
    test_enum_decoding(&path);
}

#[test]
fn test_strip_16bit_3() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/bggn4a16.png";
    test_decoding(&path);
    test_into_decoding(&path);
    test_enum_decoding(&path);
}

#[test]
fn test_strip_16bit_4() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn2c16.png";
    test_decoding(&path);
    test_into_decoding(&path);
    test_enum_decoding(&path);
}
