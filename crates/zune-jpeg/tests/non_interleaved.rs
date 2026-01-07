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

/// Test decoding a small non-interleaved 4:4:4 baseline JPEG (16x16).
///
/// This tiny image has 3 separate SOS markers (one per Y, Cb, Cr component).
/// Note: Very small images (single MCU) may decode correctly even with buggy
/// non-interleaved handling due to special case behavior.
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

/// Test decoding a larger non-interleaved 4:4:4 baseline JPEG (64x64).
///
/// This 64x64 test image has 3 separate SOS markers (one per Y, Cb, Cr).
/// With multiple MCUs (8x8 blocks), this properly tests the non-interleaved
/// scan handling logic.
///
/// The image is a gradient: red increases left-to-right, green top-to-bottom.
#[test]
fn decode_non_interleaved_64x64() {
    let test_data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect(
        "Failed to decode 64x64 non-interleaved JPEG - \
         decoder likely doesn't handle multiple SOS markers correctly"
    );

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 64, "Expected width 64");
    assert_eq!(info.height, 64, "Expected height 64");
    assert_eq!(pixels.len(), 64 * 64 * 3, "Unexpected pixel count");

    // Verify we got colorful output, not all gray
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
        "Too many gray pixels ({:.1}%): non-interleaved decoding produced wrong colors",
        gray_ratio * 100.0
    );

    // Verify gradient pattern: red should increase with x
    let tl_r = pixels[0]; // top-left red
    let tr_r = pixels[(63) * 3]; // top-right red
    assert!(
        tr_r > tl_r + 100,
        "Red gradient not detected: left R={}, right R={}",
        tl_r,
        tr_r
    );
}

/// Test that 4:2:0 non-interleaved images decode without panicking.
///
/// Note: 4:2:0 non-interleaved requires upsampling the chroma components
/// from 32x32 to 64x64. This is currently not fully implemented, so the
/// output may have color issues (black regions in lower half).
///
/// This test only verifies that decoding completes successfully.
#[test]
fn decode_non_interleaved_420_64x64_no_panic() {
    let test_data = include_bytes!("../../../test-images/jpeg/non_interleaved_64x64.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode 4:2:0 non-interleaved JPEG");

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);
    assert_eq!(pixels.len(), 64 * 64 * 3);

    // Just verify we got some non-black pixels (at least the top rows should have color)
    let non_black = pixels.chunks(3).filter(|c| c[0] > 0 || c[1] > 0 || c[2] > 0).count();
    assert!(non_black > 0, "Expected some non-black pixels");
}
