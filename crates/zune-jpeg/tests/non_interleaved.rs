/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for non-interleaved JPEG decoding.
//!
//! Non-interleaved JPEGs have separate SOS (Start Of Scan) markers for each
//! color component, rather than interleaving all components in a single scan.
//! This is valid per ITU-T.81 (JPEG standard) Section B.2.3.
//!
//! These images are created with tools like libjpeg's cjpeg -scans option.

use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

/// Test decoding a non-interleaved 4:4:4 baseline JPEG (64x64).
///
/// This test image has 3 separate SOS markers (one per Y, Cb, Cr component).
/// With multiple MCUs (8x8 blocks), this properly tests the non-interleaved
/// scan handling logic including DHT markers between scans.
#[test]
fn decode_non_interleaved_444_64x64() {
    let test_data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect(
        "Failed to decode 64x64 non-interleaved JPEG - \
         decoder likely doesn't handle DHT markers between scans"
    );

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 64, "Expected width 64");
    assert_eq!(info.height, 64, "Expected height 64");
    assert_eq!(pixels.len(), 64 * 64 * 3, "Unexpected pixel count");

    // Verify we got colorful output, not all gray.
    // A broken decoder fills missing chroma with 128, producing gray pixels.
    let gray_count = pixels
        .chunks(3)
        .filter(|c| {
            let r = c[0] as i32;
            let g = c[1] as i32;
            let b = c[2] as i32;
            (r - g).abs() <= 10 && (r - b).abs() <= 10
        })
        .count();

    let total_pixels = pixels.len() / 3;
    let gray_ratio = gray_count as f64 / total_pixels as f64;

    assert!(
        gray_ratio < 0.5,
        "Too many gray pixels ({:.1}%): non-interleaved decoding likely failed to process chroma",
        gray_ratio * 100.0
    );
}

/// Test decoding a small non-interleaved 4:4:4 baseline JPEG (16x16).
#[test]
fn decode_non_interleaved_16x16() {
    let test_data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode 16x16 non-interleaved JPEG");

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 16);
    assert_eq!(info.height, 16);
    assert_eq!(pixels.len(), 16 * 16 * 3);
}

/// Test decoding a 4:2:0 non-interleaved JPEG.
#[test]
fn decode_non_interleaved_420_64x64() {
    let test_data = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode 4:2:0 non-interleaved JPEG");

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);
    assert_eq!(pixels.len(), 64 * 64 * 3);

    // All pixels should have color (no large black regions from failed upsampling)
    let non_black = pixels.chunks(3).filter(|c| c[0] > 5 || c[1] > 5 || c[2] > 5).count();
    let total_pixels = pixels.len() / 3;
    let non_black_ratio = non_black as f64 / total_pixels as f64;
    assert!(
        non_black_ratio > 0.90,
        "Expected >90% non-black pixels, got {:.1}%",
        non_black_ratio * 100.0
    );
}

/// Test decoding a 4:2:2 non-interleaved JPEG (horizontal-only subsampling).
#[test]
fn decode_non_interleaved_422_64x64() {
    let test_data = include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode 4:2:2 non-interleaved JPEG");

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);
    assert_eq!(pixels.len(), 64 * 64 * 3);
}

/// Test decoding a 4:4:0 non-interleaved JPEG (vertical-only subsampling).
#[test]
fn decode_non_interleaved_440_64x64() {
    let test_data = include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode 4:4:0 non-interleaved JPEG");

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);
    assert_eq!(pixels.len(), 64 * 64 * 3);
}

/// Test that the existing sos_news.jpeg (non-interleaved) still works.
#[test]
fn decode_sos_news_non_interleaved() {
    let test_data = include_bytes!("../../../test-images/jpeg/sos_news.jpeg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode sos_news.jpeg");

    let info = decoder.info().expect("Failed to get image info");
    assert!(info.width > 0);
    assert!(info.height > 0);
    assert_eq!(pixels.len(), info.width as usize * info.height as usize * 3);
}

/// Test decoding a 4:2:2 non-interleaved JPEG with non-MCU-aligned width (65x65).
#[test]
fn decode_non_interleaved_422_65x65() {
    let test_data = include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode 4:2:2 non-interleaved JPEG");

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 65);
    assert_eq!(info.height, 65);
    assert_eq!(pixels.len(), 65 * 65 * 3);

    // Test image has pattern R=x*3, G=y*3, B=(x+y)*2. Check center pixel (32,32)
    // is approximately (96,96,128) and has not been shifted.
    let idx = (32 * 65 + 32) * 3;
    let (r, g, b) = (pixels[idx] as i16, pixels[idx + 1] as i16, pixels[idx + 2] as i16);
    assert!(
        (r - 96).abs() <= 20 && (g - 96).abs() <= 20 && (b - 128).abs() <= 20,
        "Center pixel wrong: expected ~(96,96,128), got ({},{},{})",
        r, g, b
    );
}
