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

use zune_core::bytestream::ZCursor;
use zune_core::result::DecodingResult;

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
    read(path).unwrap()
}

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let transformations = png::Transformations::EXPAND;

    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(transformations);
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
fn test_16bpp_to_8bpp_interlaced() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/bgai4a16.png";

    let contents = open_and_read(path);
    let opt = zune_core::options::DecoderOptions::new_fast().png_set_strip_to_8bit(true);
    let mut decoder = zune_png::PngDecoder::new_with_options(ZCursor::new(&contents), opt);
    let normal_decode = zune_png::PngDecoder::new(ZCursor::new(&contents))
        .decode()
        .unwrap();

    let strip_decode = decoder.decode().unwrap().u8().unwrap();
    if let DecodingResult::U16(new) = normal_decode {
        for (pos, (a, b)) in new.iter().zip(strip_decode).enumerate() {
            if (a >> 8) as u8 != b {
                panic!("{pos}= ({:?} != {:?})", (a >> 8), b);
            }
        }
    }
}
#[test]
fn test_16bpp_to_8bpp_basic() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn6a16.png";

    let contents = open_and_read(path);
    let opt = zune_core::options::DecoderOptions::new_fast().png_set_strip_to_8bit(true);
    let mut decoder = zune_png::PngDecoder::new_with_options(ZCursor::new(&contents), opt);
    let normal_decode = zune_png::PngDecoder::new(ZCursor::new(&contents))
        .decode()
        .unwrap();

    let strip_decode = decoder.decode().unwrap().u8().unwrap();
    if let DecodingResult::U16(new) = normal_decode {
        for (pos, (a, b)) in new.iter().zip(strip_decode).enumerate() {
            if (a >> 8) as u8 != b {
                panic!("{pos}= ({:?} != {:?})", (a >> 8), b);
            }
        }
    }
}

#[test]
fn test_1bpp_basic_interlaced() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi0g01.png";

    test_decoding(path);
}

#[test]
fn test_2bpp_basic() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn0g02.png";

    test_decoding(path);
}

#[test]
fn test_2bpp_basic_interlaced() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi0g02.png";

    test_decoding(path);
}

#[test]
fn test_4bpp_basic() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn0g04.png";

    test_decoding(path);
}

#[test]
fn test_4bpp_basic_interlaced() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi0g04.png";

    test_decoding(path);
}

#[test]
fn test_8bpp_basic() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn0g08.png";

    test_decoding(path);
}

#[test]
fn test_16bpp_basic() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn0g16.png";

    test_decoding(path);
}

#[test]
fn test_8bpp_luma_basic() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn2c08.png";

    test_decoding(path);
}

#[test]
fn test_16bpp_luma_basic() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn2c16.png";

    test_decoding(path);
}

#[test]
fn test_16bpp_interlaced_color() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi2c16.png";

    test_decoding(path);
}

#[test]
fn test_basn6a16() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basn6a16.png";

    test_decoding(path);
}
