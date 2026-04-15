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

fn decode_oneshot(data: &[u8]) -> Vec<u8> {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode().expect("one-shot decode failed")
}

/// Feeding the entire file at once via decode_headers + decode_into must
/// produce byte-identical output to decode() — no regressions.
#[test]
fn full_decode_unchanged() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);

    let mut decoder = JpegDecoder::new(ZCursor::new(&data[..]));
    decoder.decode_headers().unwrap();
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut out).unwrap();
    assert_eq!(out, expected);
}

/// Truncated input must return a recoverable EOF from decode_headers.
/// Non-EOF errors (bad magic, format errors) must NOT be recoverable.
#[test]
fn incomplete_data_returns_recoverable_eof() {
    // Empty input → recoverable EOF
    let mut dec = JpegDecoder::new(ZCursor::new(&[] as &[u8]));
    let err = dec.decode_headers().unwrap_err();
    assert!(err.is_recoverable_eof(), "empty input: expected recoverable EOF, got: {err:?}");

    // Truncated just after SOI → recoverable EOF
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(&data[..2]));
    let err = dec.decode_headers().unwrap_err();
    assert!(err.is_recoverable_eof(), "truncated after SOI: expected recoverable EOF, got: {err:?}");

    // Bad magic bytes → NOT recoverable
    let mut dec = JpegDecoder::new(ZCursor::new(&[0x00, 0x00]));
    let err = dec.decode_headers().unwrap_err();
    assert!(!err.is_recoverable_eof(), "bad magic: should not be recoverable, got: {err:?}");

    // Non-EOF error variants → NOT recoverable
    use zune_jpeg::errors::DecodeErrors;
    assert!(!DecodeErrors::ZeroError.is_recoverable_eof());
    assert!(!DecodeErrors::FormatStatic("bad").is_recoverable_eof());
}

/// Core resumable decoding test: decode_headers on truncated data returns
/// a recoverable EOF, then a new decoder with the full data succeeds and
/// produces byte-identical output to a one-shot decode.
#[test]
fn resumable_decode_after_eof() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);

    // Step 1: feed only a small prefix — not enough for headers.
    let truncated = &data[..64];
    let mut decoder = JpegDecoder::new(ZCursor::new(truncated));
    let err = decoder.decode_headers().unwrap_err();
    assert!(err.is_recoverable_eof(), "expected recoverable EOF on truncated data");

    // Step 2: "fill the stream" — create a new decoder with the full data
    // (ZCursor<&[u8]> is immutable, so we simulate refill by recreating).
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    let result = decoder.decode().unwrap();
    assert_eq!(result, expected, "full decode after EOF must match one-shot");
}

/// Calling decode_headers repeatedly on the same decoder with insufficient
/// data must not panic or corrupt state — each attempt resets cleanly.
#[test]
fn repeated_eof_does_not_corrupt_state() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");

    let mut decoder = JpegDecoder::new(ZCursor::new(&data[..10]));

    for _ in 0..5 {
        let err = decoder.decode_headers().unwrap_err();
        assert!(err.is_recoverable_eof(), "expected recoverable EOF on retry, got: {err:?}");
        assert!(decoder.info().is_none(), "info() must be None after failed headers");
    }
}

/// Byte-by-byte header feeding: grow the visible window one byte at a time,
/// creating a new decoder each time, until headers succeed. The final decoded
/// output must match one-shot.
#[test]
fn chunked_header_feeding_byte_by_byte() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let expected = decode_oneshot(data);

    let mut available = 1;
    loop {
        let mut decoder = JpegDecoder::new(ZCursor::new(&data[..available]));
        match decoder.decode_headers() {
            Ok(()) => {
                // Headers succeeded — now decode with full data.
                let mut decoder = JpegDecoder::new(ZCursor::new(data));
                decoder.decode_headers().unwrap();
                let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
                decoder.decode_into(&mut out).unwrap();
                assert_eq!(out, expected);
                return;
            }
            Err(ref e) if e.is_recoverable_eof() => {
                available += 1;
                assert!(available <= data.len(), "EOF with all bytes available");
            }
            Err(e) => panic!("unexpected error at byte {available}: {e:?}"),
        }
    }
}
