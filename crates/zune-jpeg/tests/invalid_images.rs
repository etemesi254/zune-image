/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

#[test]
fn eof() {
    let mut decoder = JpegDecoder::new(ZCursor::new([0xff, 0xd8, 0xa4]));

    decoder.decode().unwrap_err();
}

#[test]
fn bad_ff_marker_size() {
    let mut decoder = JpegDecoder::new(ZCursor::new([0xff, 0xd8, 0xff, 0x00, 0x00, 0x00]));

    let _ = decoder.decode().unwrap_err();
}

#[test]
fn bad_number_of_scans() {
    let mut decoder = JpegDecoder::new(ZCursor::new([255, 216, 255, 218, 232, 197, 255]));

    let err = decoder.decode().unwrap_err();

    assert!(
        matches!(err, zune_jpeg::errors::DecodeErrors::SosError(x) if x == "Bad SOS length 59589,corrupt jpeg")
    );
}

#[test]
fn huffman_length_subtraction_overflow() {
    let mut decoder = JpegDecoder::new(ZCursor::new([255, 216, 255, 196, 0, 0]));

    let err = decoder.decode().unwrap_err();

    assert!(
        matches!(err, zune_jpeg::errors::DecodeErrors::FormatStatic(x) if x == "Invalid Huffman length in image")
    );
}

#[test]
fn index_oob() {
    let mut decoder = JpegDecoder::new(ZCursor::new([255, 216, 255, 218, 0, 8, 1, 0, 8, 1]));

    let _ = decoder.decode().unwrap_err();
}

#[test]
fn mul_with_overflow() {
    let mut decoder = JpegDecoder::new(ZCursor::new([
        255, 216, 255, 192, 255, 1, 8, 9, 119, 48, 255, 192
    ]));

    let err = decoder.decode().unwrap_err();

    assert!(
        matches!(err, zune_jpeg::errors::DecodeErrors::SofError(x) if x == "Length of start of frame differs from expected 584,value is 65281")
    );
}
