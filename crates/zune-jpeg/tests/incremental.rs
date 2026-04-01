/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for incremental (resumable) JPEG decoding.
//!
//! Verifies that `is_recoverable_eof()` is reported on truncated input
//! and that retrying with more data produces correct output.

use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

/// Find the smallest prefix of `data` for which `decode_headers` succeeds.
/// Returns the length of that prefix.
fn find_header_boundary(data: &[u8]) -> usize {
    for n in 1..=data.len() {
        let mut dec = JpegDecoder::new(ZCursor::new(&data[..n]));
        if dec.decode_headers().is_ok() {
            return n;
        }
    }
    panic!("decode_headers never succeeded");
}

/// Feed `data` to the decoder in chunks of `chunk_size` bytes for the
/// header phase, growing the visible window one chunk at a time.
/// Each time `decode_headers` returns a recoverable EOF, a **new decoder**
/// is created with a larger slice (since `ZCursor<&[u8]>` is immutable).
/// Once headers are decoded, the scan is run with the full data.
///
/// Returns the decoded pixels.
fn decode_chunked_headers(data: &[u8], chunk_size: usize) -> Vec<u8> {
    assert!(chunk_size > 0);

    let mut available = chunk_size.min(data.len());

    // Phase 1: decode headers, growing the window until success
    let mut decoder;
    loop {
        decoder = JpegDecoder::new(ZCursor::new(&data[..available]));
        match decoder.decode_headers() {
            Ok(()) => break,
            Err(ref e) if e.is_recoverable_eof() => {
                if available >= data.len() {
                    panic!(
                        "decode_headers: EOF with all {} bytes available",
                        data.len()
                    );
                }
                available = (available + chunk_size).min(data.len());
                // Create a new decoder with the larger slice on next iteration
            }
            Err(e) => panic!("decode_headers error at available={available}: {e:?}"),
        }
    }

    // Phase 2: headers succeeded; if the decoder doesn't have the full
    // data yet, create a final decoder with everything.
    if available < data.len() {
        decoder = JpegDecoder::new(ZCursor::new(data));
        decoder.decode_headers().unwrap();
    }

    let size = decoder.output_buffer_size().unwrap();
    let mut out = vec![0u8; size];
    decoder.decode_into(&mut out).unwrap();
    out
}

fn decode_oneshot(data: &[u8]) -> Vec<u8> {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode().expect("one-shot decode failed")
}

/// Feeding the entire file at once must still work — no regressions.
#[test]
fn full_decode_unchanged() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);
    // Use decode_headers + decode_into explicitly.
    let mut decoder = JpegDecoder::new(ZCursor::new(&data[..]));
    decoder.decode_headers().unwrap();
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut out).unwrap();
    assert_eq!(out, expected);
}

/// Chunked header decode with 100-byte pieces must produce byte-identical output.
#[test]
fn chunked_headers_100_bytes() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);
    let actual = decode_chunked_headers(data, 100);
    assert_eq!(actual, expected);
}

/// Chunked header decode with 1-byte pieces (stress test on a tiny image).
#[test]
fn chunked_headers_1_byte() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let expected = decode_oneshot(data);
    let actual = decode_chunked_headers(data, 1);
    assert_eq!(actual, expected);
}

/// Chunk larger than the file — never triggers a recoverable EOF.
#[test]
fn chunked_headers_larger_than_file() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);
    let actual = decode_chunked_headers(data, data.len() + 1);
    assert_eq!(actual, expected);
}

/// Progressive JPEG: chunked header decode must produce the same output.
#[test]
fn chunked_headers_progressive() {
    let data =
        include_bytes!("../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg");
    let expected = decode_oneshot(data);
    let actual = decode_chunked_headers(data, 512);
    assert_eq!(actual, expected);
}

/// JPEG with restart markers: chunked header decode.
#[test]
fn chunked_headers_restart_markers() {
    let data = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let expected = decode_oneshot(data);
    let actual = decode_chunked_headers(data, 256);
    assert_eq!(actual, expected);
}

/// Empty input must return a recoverable EOF error from decode_headers.
#[test]
fn empty_input_is_recoverable_eof() {
    let mut decoder = JpegDecoder::new(ZCursor::new(&[] as &[u8]));
    let err = decoder.decode_headers().unwrap_err();
    assert!(
        err.is_recoverable_eof(),
        "expected recoverable EOF, got: {err:?}"
    );
}

/// Non-interleaved JPEG: chunked header decode.
#[test]
fn chunked_headers_non_interleaved() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let expected = decode_oneshot(data);
    let actual = decode_chunked_headers(data, 200);
    assert_eq!(actual, expected);
}

/// Down-sampled image (4:2:0): chunked header decode.
#[test]
fn chunked_headers_420_subsampled() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let expected = decode_oneshot(data);
    let actual = decode_chunked_headers(data, 300);
    assert_eq!(actual, expected);
}

/// `is_recoverable_eof` returns false for non-EOF errors.
#[test]
fn non_eof_errors_are_not_recoverable() {
    use zune_jpeg::errors::DecodeErrors;
    assert!(!DecodeErrors::ZeroError.is_recoverable_eof());
    assert!(!DecodeErrors::IllegalMagicBytes(0x1234).is_recoverable_eof());
    assert!(!DecodeErrors::FormatStatic("bad").is_recoverable_eof());
}

/// After decode_headers recovers from EOF, the header boundary is correct:
/// we can get the exact same position where scan data starts.
#[test]
fn header_boundary_is_consistent() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");

    // Find the minimal amount of data needed for headers to succeed.
    let boundary = find_header_boundary(data);
    assert!(boundary > 2, "header boundary should be past SOI");
    assert!(boundary < data.len(), "header boundary should be before EOF");

    // Verify that feeding exactly `boundary` bytes works via chunked headers.
    let expected = decode_oneshot(data);
    let actual = decode_chunked_headers(data, boundary);
    assert_eq!(actual, expected);
}

/// After a recoverable EOF, creating a new decoder with more data
/// and decoding from scratch must produce the correct result.
#[test]
fn retry_with_new_decoder() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);

    // Start with too little data (just the SOI marker)
    let mut decoder = JpegDecoder::new(ZCursor::new(&data[..2]));
    let err = decoder.decode_headers().unwrap_err();
    assert!(err.is_recoverable_eof());

    // Create a new decoder with all data
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();

    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut out).unwrap();
    assert_eq!(out, expected);
}

/// Calling `decode_headers` repeatedly on the same decoder after
/// recoverable EOF must not panic or corrupt state — each attempt
/// resets cleanly and re-parses from position 0.
#[test]
fn repeated_eof_on_same_decoder_does_not_corrupt() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");

    // Create decoder with very little data (just SOI + a few bytes)
    let mut decoder = JpegDecoder::new(ZCursor::new(&data[..10]));

    // Multiple retries on the same decoder must all return recoverable EOF
    // without panicking — proves reset_header_state works correctly.
    for _ in 0..5 {
        let err = decoder.decode_headers().unwrap_err();
        assert!(
            err.is_recoverable_eof(),
            "expected recoverable EOF on retry, got: {err:?}"
        );
        // info() must return None since headers were never fully parsed
        assert!(decoder.info().is_none());
    }
}
