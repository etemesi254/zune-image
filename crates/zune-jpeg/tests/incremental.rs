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

use std::cell::Cell;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::rc::Rc;

/// A cursor over a byte slice with an adjustable visibility limit.
///
/// Reads/seeks beyond `limit` behave as if the data ends there (EOF).
/// The test can grow `limit` via the shared `Rc<Cell<usize>>` to
/// simulate more data arriving, then retry on the **same** decoder.
struct GrowableCursor<'a> {
    data:     &'a [u8],
    position: usize,
    limit:    Rc<Cell<usize>>,
}

impl<'a> GrowableCursor<'a> {
    fn new(data: &'a [u8], limit: Rc<Cell<usize>>) -> Self {
        Self { data, position: 0, limit }
    }

    fn visible(&self) -> usize {
        self.limit.get().min(self.data.len())
    }
}

impl Read for GrowableCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let visible = self.visible();
        if self.position >= visible {
            return Ok(0); // EOF
        }
        let available = &self.data[self.position..visible];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.position += n;
        Ok(n)
    }
}

impl BufRead for GrowableCursor<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let visible = self.visible();
        if self.position >= visible {
            return Ok(&[]);
        }
        Ok(&self.data[self.position..visible])
    }

    fn consume(&mut self, amt: usize) {
        self.position += amt;
    }
}

impl Seek for GrowableCursor<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::Current(p) => self.position as i64 + p,
            SeekFrom::End(p) => self.visible() as i64 + p,
        };
        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }
}

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

/// Byte-by-byte header feeding with a *fresh* decoder per attempt: this is
/// a one-shot parity regression test, not a fine-grained resume test (each
/// attempt restarts from SOI).
#[test]
fn chunked_fresh_decoder_byte_by_byte() {
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

/// In-place resumable header parsing on the *same* decoder instance.
///
/// Uses `GrowableCursor` to simulate a stream where more data becomes
/// available over time.  The decoder must resume from where it left off
/// (not restart from SOI) and eventually succeed, producing output that
/// matches a one-shot decode.
#[test]
fn inplace_growable_header_resume() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    // Grow limit in chunks until headers succeed.
    let step = 32;
    loop {
        let new_limit = (limit.get() + step).min(data.len());
        limit.set(new_limit);

        match decoder.decode_headers() {
            Ok(()) => break,
            Err(ref e) if e.is_recoverable_eof() => {
                assert!(
                    new_limit < data.len(),
                    "EOF with all bytes visible — headers should have succeeded"
                );
            }
            Err(e) => panic!("unexpected error at limit {new_limit}: {e:?}"),
        }
    }

    // Headers decoded — make all data visible and decode the image.
    limit.set(data.len());
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut out).unwrap();
    assert_eq!(out, expected, "decoded pixels must match one-shot decode");
}

/// Byte-by-byte in-place resume: grow visibility one byte at a time on
/// the *same* decoder instance, verifying that fine-grained checkpointing
/// works for every possible truncation point in the headers.
#[test]
fn inplace_byte_by_byte_header_resume() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let expected = decode_oneshot(data);

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    for avail in 1..=data.len() {
        limit.set(avail);

        match decoder.decode_headers() {
            Ok(()) => {
                // Headers succeeded — make all data visible and decode.
                limit.set(data.len());
                let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
                decoder.decode_into(&mut out).unwrap();
                assert_eq!(out, expected);
                return;
            }
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(e) => panic!("unexpected error at byte {avail}: {e:?}"),
        }
    }
    panic!("headers never succeeded even with all bytes visible");
}

/// Verify that an EOF after a completed SOF marker is recoverable and that
/// supplying the rest of the data lets the decoder finish parsing headers.
///
/// Note: this asserts the resume *succeeds*, not that markers parsed before
/// the EOF were preserved across the retry. The byte-by-byte in-place tests
/// above exercise true fine-grained checkpointing.
#[test]
fn resume_after_sof_eventually_succeeds() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");

    // Find roughly where SOF is by scanning for 0xFFC0 marker.
    let sof_pos = data
        .windows(2)
        .position(|w| w == [0xFF, 0xC0] || w == [0xFF, 0xC2])
        .expect("no SOF marker found in test image");

    // Find roughly where SOS is by scanning for 0xFFDA marker.
    let sos_pos = data
        .windows(2)
        .position(|w| w == [0xFF, 0xDA])
        .expect("no SOS marker found in test image");

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    // Feed enough data to get past SOF but not past SOS.
    // We pick a point between SOF and SOS.  The SOF marker segment
    // is typically ~20 bytes, so SOF + 30 should be safely past it.
    let mid = (sof_pos + 30).min(sos_pos);
    limit.set(mid);
    let err = decoder.decode_headers().unwrap_err();
    assert!(err.is_recoverable_eof(), "expected EOF between SOF and SOS");
    // Headers haven't completed yet, so info() must still be None.
    assert!(decoder.info().is_none(), "info() must be None until headers complete");

    // Now supply all data; the decoder should resume and finish.
    limit.set(data.len());
    decoder.decode_headers().unwrap();

    let info = decoder.info().expect("info() must be Some after headers");
    assert!(info.width > 0 && info.height > 0, "dimensions must be valid");
}

/// Build a synthetic APP2 ICC chunk with a single payload segment.
fn icc_app2_chunk(payload: &[u8]) -> Vec<u8> {
    let body_len = 2 + 12 + 1 + 1 + payload.len();
    assert!(body_len <= u16::MAX as usize, "payload too large for one APP2");

    let mut chunk = Vec::with_capacity(2 + body_len);
    chunk.extend_from_slice(&[0xFF, 0xE2]);
    chunk.extend_from_slice(&(body_len as u16).to_be_bytes());
    chunk.extend_from_slice(b"ICC_PROFILE\0");
    chunk.push(1);
    chunk.push(1);
    chunk.extend_from_slice(payload);
    chunk
}

/// Inject a synthetic APP2 ICC chunk before SOS so header parsing sees it.
fn inject_header_icc(base: &[u8], payload: &[u8]) -> Vec<u8> {
    let sos = base
        .windows(2)
        .position(|w| w == [0xFF, 0xDA])
        .expect("base JPEG must contain SOS");
    let chunk = icc_app2_chunk(payload);

    let mut out = Vec::with_capacity(base.len() + chunk.len());
    out.extend_from_slice(&base[..sos]);
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&base[sos..]);
    out
}

/// Inject a synthetic APP2 ICC chunk just before the EOI marker of `base`.
///
/// The resulting JPEG places an APP marker at a position the entropy decoder
/// encounters while reading scan data; in non-strict mode the scan loop
/// dispatches such markers through `parse_marker_inner` from `mcu.rs`.
fn inject_inline_icc(base: &[u8], payload: &[u8]) -> Vec<u8> {
    let eoi = base
        .windows(2)
        .rposition(|w| w == [0xFF, 0xD9])
        .expect("base JPEG must end with EOI");
    let chunk = icc_app2_chunk(payload);

    let mut out = Vec::with_capacity(base.len() + chunk.len());
    out.extend_from_slice(&base[..eoi]);
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&base[eoi..]);
    out
}

#[test]
fn header_app2_truncation_is_recoverable() {
    let base = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let payload = b"HEADER-ICC-PAYLOAD-FOR-RESUME-TEST";
    let data = inject_header_icc(base, payload);

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(&data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    for avail in 1..=data.len() {
        limit.set(avail);

        match decoder.decode_headers() {
            Ok(()) => {
                let got_icc = decoder
                    .icc_profile()
                    .expect("incremental headers must expose injected ICC profile");
                assert_eq!(got_icc, payload);
                return;
            }
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(e) => panic!("unexpected header error at byte {avail}: {e:?}"),
        }
    }

    panic!("headers never completed even with all bytes visible");
}

/// Sanity check: byte-by-byte incremental decode of a normal image (no
/// inline markers) must reproduce the one-shot pixel output. If this test
/// fails, the bug is in the scan-retry mechanism itself rather than in
/// the inline-marker path.
#[test]
#[ignore = "scan-phase resume not yet implemented: entropy decoder treats EOF as end-of-scan"]
fn inplace_byte_by_byte_full_decode() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let expected = decode_oneshot(data);

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    let mut out: Vec<u8> = Vec::new();
    for avail in 1..=data.len() {
        limit.set(avail);

        match decoder.decode_headers() {
            Ok(()) => {}
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(e) => panic!("unexpected header error at byte {avail}: {e:?}"),
        }

        if out.is_empty() {
            out = vec![0u8; decoder.output_buffer_size().unwrap()];
        }

        match decoder.decode_into(&mut out) {
            Ok(()) => {
                assert_eq!(out, expected, "pixel output must match one-shot");
                return;
            }
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(e) => panic!("unexpected scan error at byte {avail}: {e:?}"),
        }
    }
    panic!("decode never completed");
}

/// Regression test for the case when scan-phase resume re-seeks to
/// `scan_start_position` and replays the scan, any inline marker dispatched
///  through `mcu.rs::parse_marker_inner` gets re-parsed on every retry.
/// For append-only metadata like ICC, this silently duplicates entries.
///
/// The test:
///   1. builds a synthetic JPEG with one inline APP2 ICC chunk just before EOI,
///   2. one-shot decodes it to record the expected ICC and pixel output,
///   3. incrementally decodes it byte-by-byte through `GrowableCursor`,
///      forcing many scan retries,
///   4. asserts the final ICC profile is byte-identical (not duplicated)
///      and pixels match.
#[test]
#[ignore = "scan-phase resume not yet implemented: entropy decoder treats EOF as end-of-scan"]
fn inline_marker_in_scan_does_not_duplicate_icc() {
    let base = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let payload = b"INLINE-ICC-PAYLOAD-FOR-REGRESSION-TEST";
    let data = inject_inline_icc(base, payload);

    // Sanity: one-shot decode must accept the synthetic image and surface
    // exactly the payload we injected.
    let mut oneshot = JpegDecoder::new(ZCursor::new(&data[..]));
    let expected_pixels = oneshot.decode().expect("one-shot decode of synthetic image");
    let expected_icc = oneshot
        .icc_profile()
        .expect("one-shot decode must expose injected ICC profile");
    assert_eq!(
        expected_icc, payload,
        "injected payload not round-tripped by one-shot decoder; \
         test image construction is wrong"
    );

    // Incremental decode on the *same* decoder, growing visibility one byte
    // at a time so the scan path repeatedly hits recoverable EOF and retries.
    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(&data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    let mut out: Vec<u8> = Vec::new();
    for avail in 1..=data.len() {
        limit.set(avail);

        // Drive headers first; only attempt scan once headers complete.
        match decoder.decode_headers() {
            Ok(()) => {}
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(e) => panic!("unexpected header error at byte {avail}: {e:?}"),
        }

        if out.is_empty() {
            out = vec![0u8; decoder.output_buffer_size().unwrap()];
        }

        match decoder.decode_into(&mut out) {
            Ok(()) => {
                assert_eq!(out, expected_pixels, "pixel output must match one-shot");
                let got_icc = decoder
                    .icc_profile()
                    .expect("incremental decode must expose injected ICC profile");
                assert_eq!(
                    got_icc, expected_icc,
                    "ICC profile after incremental decode must match one-shot \
                     (duplicated entries indicate scan-retry re-parses an \
                     inline marker without rolling back append-only state)"
                );
                return;
            }
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(e) => panic!("unexpected scan error at byte {avail}: {e:?}"),
        }
    }
    panic!("decode never completed even with all bytes visible");
}
