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

fn pixel_digest(pixels: &[u8]) -> u64 {
    pixels.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

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
    assert_eq!(pixel_digest(&pixels), 0xcb8faa9b4c94a0e5);

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
    assert_eq!(pixel_digest(&pixels), 0xcb8faa9b4c94a0e5);
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
        "Center pixel wrong: expected ~(96,96,128), got ({r},{g},{b})"
    );
}

// Test for https://github.com/etemesi254/zune-image/issues/419.
#[test]
fn decode_baseline_non_interleaved_420_color_balance() {
    // Path to a 4:2:0 baseline non-interleaved multi-scan JPEG (SOF0)
    let test_data = include_bytes!("../../../test-images/jpeg/baseline_non_interleaved.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    // 1. Check Slice Bounds Abort:
    // Without the fix, Luma (Y, vertical_sample = 2) double-scales row offsets
    // and aborts halfway down the image with "Would panic on slice iteration".
    let pixels = decoder
        .decode()
        .expect("Decoder aborted on 4:2:0 baseline non-interleaved JPEG");
    let info = decoder.info().expect("Failed to get image info");
    let total_pixels = info.width as usize * info.height as usize;
    assert_eq!(pixels.len(), total_pixels * 3, "Unexpected pixel buffer size");
    let mut uninitialized_black_count = 0;
    let mut green_tint_count = 0;
    let mut valid_gray_count = 0;
    for chunk in pixels.chunks_exact(3) {
        let (r, g, b) = (chunk[0], chunk[1], chunk[2]);
        if r < 10 && g < 10 && b < 10 {
            uninitialized_black_count += 1;
        } else if g.saturating_sub(r) > 40 {
            green_tint_count += 1;
        } else if r > 10 && r.abs_diff(g) < 40 && g.abs_diff(b) < 40 {
            valid_gray_count += 1;
        }
    }
    // 2. Catch current `dev` branch bug (skips odd Luma block rows -> 50% uninitialized black pixels)
    assert_eq!(
        uninitialized_black_count, 0,
        "Found {} uninitialized black pixels ({:.1}% of image): decoder skipped Luma rows!",
        uninitialized_black_count,
        (uninitialized_black_count as f64 / total_pixels as f64) * 100.0
    );
    // 3. Catch release `0.5.15` bug (Cb over-read -> Cr starvation -> full-frame green tint)
    assert_eq!(
        green_tint_count, 0,
        "Found {} green-tinted pixels ({:.1}% of image): chroma bitstream starvation occurred!",
        green_tint_count,
        (green_tint_count as f64 / total_pixels as f64) * 100.0
    );
    // 4. Confirm 100% of pixels in the tile decoded to valid balanced gray/white
    assert_eq!(
        valid_gray_count, total_pixels,
        "Expected all {} pixels to be balanced gray, but only {} were valid.",
        total_pixels, valid_gray_count
    );
}

#[test]
fn decode_non_interleaved_with_luma_last() {
    let test_data =
        include_bytes!("../../../test-images/jpeg/non_interleaved_luma_last_88x88.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder
        .decode()
        .expect("Failed to decode non-interleaved JPEG with luma in the final scan");

    assert_eq!(pixels.len(), 88 * 88 * 3);
}

#[test]
fn read_ahead_marker_does_not_drop_final_scan_row() {
    let test_data =
        include_bytes!("../../../test-images/jpeg/non_interleaved_marker_read_ahead_88x88.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode non-interleaved JPEG");

    assert_eq!(pixels.len(), 88 * 88 * 3);
    let bottom_rows = &pixels[(80 * 88 * 3)..];
    let hash = bottom_rows.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    });
    assert_eq!(
        hash, 0x1a6e_8556_3a98_c3bc,
        "final eight rows differ from the libjpeg-turbo reference"
    );
}

/// `(restart interval, scans, single-component scans)` read from the marker stream.
fn restart_scan_layout(bytes: &[u8]) -> (usize, usize, usize) {
    let mut position = 2;
    let mut restart_interval = 0;
    let mut scans = 0;
    let mut single_component_scans = 0;
    while position + 1 < bytes.len() {
        if bytes[position] != 0xFF {
            position += 1;
            continue;
        }
        let marker = bytes[position + 1];
        position += 2;
        if marker == 0xD9 {
            break;
        }
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if position + 2 > bytes.len() {
            break;
        }
        let length = (usize::from(bytes[position]) << 8) | usize::from(bytes[position + 1]);
        if length < 2 || position + length > bytes.len() {
            break;
        }
        let segment = &bytes[position + 2..position + length];
        match marker {
            0xDD => restart_interval = (usize::from(segment[0]) << 8) | usize::from(segment[1]),
            0xDA => {
                scans += 1;
                if segment.first() == Some(&1) {
                    single_component_scans += 1;
                }
                position += length;
                // Skip entropy-coded data, including stuffed bytes and RST markers.
                while position + 1 < bytes.len() {
                    if bytes[position] == 0xFF {
                        let next = bytes[position + 1];
                        if next == 0x00 || (0xD0..=0xD7).contains(&next) {
                            position += 2;
                            continue;
                        }
                        break;
                    }
                    position += 1;
                }
                continue;
            }
            _ => {}
        }
        position += length;
    }
    (restart_interval, scans, single_component_scans)
}

#[test]
fn restart_interval_resets_at_each_non_interleaved_scan() {
    // 333x257 4:2:2, optimized Huffman tables, DRI 4, one component per SOS. The luma scan's
    // data-unit count is not a multiple of 4, so a countdown carried into the chroma scans
    // shifts every later RST boundary.
    let test_data =
        include_bytes!("../../../test-images/jpeg/restart_non_interleaved_422_333x257.jpg");
    assert_eq!(
        restart_scan_layout(test_data),
        (4, 3, 3),
        "fixture must be non-interleaved with a restart interval"
    );

    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode JPEG with per-scan restarts");
    let info = decoder.info().expect("Failed to get image info");
    assert_eq!((info.width, info.height), (333, 257));
    assert_eq!(pixels.len(), 333 * 257 * 3);

    // Compare against the exact source the encoder was given (a gradient, so the reference is
    // generated rather than stored). libjpeg-turbo decodes this fixture at 1.41 mean absolute
    // error; the leaked countdown put zune at 38.9 before the reset.
    let mut source = Vec::with_capacity(pixels.len());
    for y in 0..257u32 {
        for x in 0..333u32 {
            source.push((x * 255 / 332) as u8);
            source.push((y * 255 / 256) as u8);
            source.push(((x ^ y) & 0xFF) as u8);
        }
    }
    let total_error: u64 = pixels
        .iter()
        .zip(source.iter())
        .map(|(decoded, expected)| u64::from(decoded.abs_diff(*expected)))
        .sum();
    let mean_absolute_error = total_error as f64 / pixels.len() as f64;
    assert!(
        mean_absolute_error < 4.0,
        "mean absolute error {mean_absolute_error:.2} against the source; restart countdown \
         leaked across scans (libjpeg-turbo: 1.41)"
    );
}
