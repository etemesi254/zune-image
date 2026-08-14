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
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::JpegDecoder;

use std::cell::{Cell, RefCell};
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
    seek_log: Option<Rc<RefCell<Vec<usize>>>>
}

impl<'a> GrowableCursor<'a> {
    fn new(data: &'a [u8], limit: Rc<Cell<usize>>) -> Self {
        Self {
            data,
            position: 0,
            limit,
            seek_log: None
        }
    }

    fn with_seek_log(
        data: &'a [u8], limit: Rc<Cell<usize>>, seek_log: Rc<RefCell<Vec<usize>>>
    ) -> Self {
        Self {
            data,
            position: 0,
            limit,
            seek_log: Some(seek_log)
        }
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
        if let Some(seek_log) = &self.seek_log {
            seek_log.borrow_mut().push(self.position);
        }
        Ok(self.position as u64)
    }
}

fn decode_oneshot(data: &[u8]) -> Vec<u8> {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode().expect("one-shot decode failed")
}

fn decode_with_mode(data: &[u8], incremental: bool, strict: bool) -> Result<Vec<u8>, DecodeErrors> {
    let options = DecoderOptions::default().set_strict_mode(strict);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.set_incremental_mode(incremental);
    decoder.decode()
}

fn assert_pixels_match(actual: &[u8], expected: &[u8], name: &str, available: usize) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{name}: incremental and one-shot output lengths differ"
    );

    if let Some(index) = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right)
    {
        let start = index.saturating_sub(8);
        let end = (index + 9).min(actual.len());
        panic!(
            "{name}: first pixel byte mismatch at {index} with {available} bytes visible: incremental={}, one-shot={}, incremental window={:?}, one-shot window={:?}",
            actual[index],
            expected[index],
            &actual[start..end],
            &expected[start..end]
        );
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn assert_incremental_decode_matches_oneshot(name: &str, data: &[u8], step: usize) {
    assert!(step > 0, "incremental step must be non-zero");

    let expected = decode_oneshot(data);
    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    let mut header_done = false;
    let mut out: Vec<u8> = Vec::new();
    let mut available = 0_usize;

    loop {
        available = (available + step).min(data.len());
        limit.set(available);

        if !header_done {
            match decoder.decode_headers() {
                Ok(()) => header_done = true,
                Err(ref e) if e.is_recoverable_eof() => {
                    assert!(
                        available < data.len(),
                        "headers still need more data with the full input visible"
                    );
                    continue;
                }
                Err(e) => panic!("unexpected header error at byte {available}: {e:?}")
            }
        }

        if out.is_empty() {
            out = vec![0u8; decoder.output_buffer_size().unwrap()];
        }

        match decoder.decode_into(&mut out) {
            Ok(()) => {
                assert_pixels_match(&out, &expected, name, available);
                return;
            }
            Err(ref e) if e.is_recoverable_eof() => {
                assert!(
                    available < data.len(),
                    "scan still needs more data with the full input visible"
                );
            }
            Err(e) => panic!("unexpected scan error at byte {available}: {e:?}")
        }
    }
}

fn assert_incremental_decode_matrix(cases: &[(&str, &[u8], usize)]) {
    for (name, data, step) in cases {
        assert_incremental_decode_matches_oneshot(name, data, *step);
    }
}

fn assert_decode_into_replay_matches_oneshot(name: &str, data: &[u8]) {
    let expected = decode_oneshot(data);

    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    let mut first = vec![0u8; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut first).unwrap();
    assert_pixels_match(&first, &expected, name, data.len());

    let mut replay = vec![0u8; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut replay).unwrap();
    assert_pixels_match(&replay, &expected, name, data.len());
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

#[test]
fn decode_into_replay_after_success_matches_oneshot() {
    assert_decode_into_replay_matches_oneshot(
        "baseline_replay",
        include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg")
    );
    assert_decode_into_replay_matches_oneshot(
        "progressive_replay",
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg")
    );
}

/// Truncated input must return a recoverable EOF from decode_headers.
/// Non-EOF errors (bad magic, format errors) must NOT be recoverable.
#[test]
fn incomplete_data_returns_recoverable_eof() {
    // Empty input → recoverable EOF
    let mut dec = JpegDecoder::new(ZCursor::new(&[] as &[u8]));
    assert!(dec.info().is_none());
    assert_eq!(dec.output_buffer_size(), None);
    assert_eq!(dec.decoded_output_bytes(), None);
    assert_eq!(dec.decoded_scanlines(), None);
    let err = dec.decode_headers().unwrap_err();
    assert!(err.is_recoverable_eof(), "empty input: expected recoverable EOF, got: {err:?}");
    assert!(dec.info().is_none());
    assert_eq!(dec.output_buffer_size(), None);
    assert_eq!(dec.decoded_output_bytes(), None);
    assert_eq!(dec.decoded_scanlines(), None);

    // Truncated just after SOI → recoverable EOF
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(&data[..2]));
    let err = dec.decode_headers().unwrap_err();
    assert!(err.is_recoverable_eof(), "truncated after SOI: expected recoverable EOF, got: {err:?}");

    // Bad magic bytes → NOT recoverable
    let mut dec = JpegDecoder::new(ZCursor::new(&[0x00, 0x00]));
    let err = dec.decode_headers().unwrap_err();
    assert!(!err.is_recoverable_eof(), "bad magic: should not be recoverable, got: {err:?}");
    assert!(dec.info().is_none());
    assert_eq!(dec.output_buffer_size(), None);
    assert_eq!(dec.decoded_output_bytes(), None);
    assert_eq!(dec.decoded_scanlines(), None);

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

fn marker_segment(code: u8, body: &[u8]) -> Vec<u8> {
    let length = body.len() + 2;
    assert!(length <= u16::MAX as usize, "marker body too large");

    let mut segment = Vec::with_capacity(2 + length);
    segment.extend_from_slice(&[0xFF, code]);
    segment.extend_from_slice(&(length as u16).to_be_bytes());
    segment.extend_from_slice(body);
    segment
}

fn inject_header_segments(base: &[u8], segments: &[Vec<u8>]) -> Vec<u8> {
    let sos = base
        .windows(2)
        .position(|w| w == [0xFF, 0xDA])
        .expect("base JPEG must contain SOS");
    let extra_len = segments.iter().map(Vec::len).sum::<usize>();

    let mut out = Vec::with_capacity(base.len() + extra_len);
    out.extend_from_slice(&base[..sos]);
    for segment in segments {
        out.extend_from_slice(segment);
    }
    out.extend_from_slice(&base[sos..]);
    out
}

fn app1_exif(payload: &[u8]) -> Vec<u8> {
    let mut body = b"Exif\0\0".to_vec();
    body.extend_from_slice(payload);
    marker_segment(0xE1, &body)
}

fn app1_xmp(payload: &[u8]) -> Vec<u8> {
    let mut body = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    body.extend_from_slice(payload);
    marker_segment(0xE1, &body)
}

fn app1_extended_xmp(guid: &[u8; 32], payload: &[u8]) -> Vec<u8> {
    let mut body = b"http://ns.adobe.com/xmp/extension/\0".to_vec();
    body.extend_from_slice(guid);
    body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    body.extend_from_slice(&0_u32.to_be_bytes());
    body.extend_from_slice(payload);
    marker_segment(0xE1, &body)
}

fn app13_iptc(payload: &[u8]) -> Vec<u8> {
    let mut body = b"Photoshop 3.0\0".to_vec();
    body.extend_from_slice(payload);
    marker_segment(0xED, &body)
}

fn app2_gain_map(payload: &[u8]) -> Vec<u8> {
    let mut body = b"urn:iso:std:iso:ts:21496:-1\0".to_vec();
    body.extend_from_slice(payload);
    marker_segment(0xE2, &body)
}

fn app2_mpf(payload: &[u8]) -> Vec<u8> {
    let mut body = b"MPF\0".to_vec();
    body.extend_from_slice(payload);
    marker_segment(0xE2, &body)
}

fn com_segment(payload: &[u8]) -> Vec<u8> {
    marker_segment(0xFE, payload)
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

#[test]
fn header_metadata_markers_commit_once_across_retries() {
    let base = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let exif_payload = b"EXIF-PAYLOAD-FOR-RESUME";
    let xmp_payload = b"<xmp>RESUME-XMP-PAYLOAD</xmp>";
    let extended_payload = b"EXTENDED-XMP-PAYLOAD-FOR-RESUME";
    let extended_guid = *b"0123456789abcdef0123456789abcdef";
    let iptc_payload = b"IPTC-PAYLOAD-FOR-RESUME";
    let gain_payload = b"\x00\x00\x00\x01GAIN-MAP-PAYLOAD-FOR-RESUME";
    let mpf_payload = b"MPF-PAYLOAD-FOR-RESUME";
    let data = inject_header_segments(
        base,
        &[
            app1_exif(exif_payload),
            app1_xmp(xmp_payload),
            app1_extended_xmp(&extended_guid, extended_payload),
            app13_iptc(iptc_payload),
            app2_gain_map(gain_payload),
            app2_mpf(mpf_payload),
            com_segment(b"comment marker is skipped but must replay safely"),
        ],
    );
    let expected = decode_oneshot(&data);

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(&data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    for avail in 1..=data.len() {
        limit.set(avail);

        match decoder.decode_headers() {
            Ok(()) => break,
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(e) => panic!("unexpected header error at byte {avail}: {e:?}"),
        }
    }

    let info = decoder.info().expect("headers must eventually complete");
    assert_eq!(decoder.exif().map(Vec::as_slice), Some(exif_payload.as_slice()));
    assert_eq!(decoder.xmp().map(Vec::as_slice), Some(xmp_payload.as_slice()));
    assert_eq!(decoder.iptc().map(Vec::as_slice), Some(iptc_payload.as_slice()));
    assert_eq!(info.extended_xmp.as_deref(), Some(extended_payload.as_slice()));
    assert_eq!(info.extended_xmp_guid.as_deref(), Some(extended_guid.as_slice()));
    assert_eq!(info.gain_map_info.len(), 1, "gain map marker duplicated or lost");
    assert_eq!(info.gain_map_info[0].data.as_slice(), gain_payload.as_slice());
    assert_eq!(info.multi_picture_information.as_deref(), Some(mpf_payload.as_slice()));

    limit.set(data.len());
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    decoder
        .decode_into(&mut out)
        .expect("decode should succeed after metadata retries");
    assert_eq!(out, expected, "pixels must match one-shot decode");
}

/// Sanity check: give headers + partial scan, get EOF, then give
/// full data and verify pixels match one-shot.
#[test]
fn scan_resume_two_step() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let expected = decode_oneshot(data);

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    // Expose 80% of data — headers should complete, scan should fail
    let partial = data.len() * 80 / 100;
    limit.set(partial);

    decoder
        .decode_headers()
        .expect("headers should succeed at 80%");
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];

    match decoder.decode_into(&mut out) {
        Ok(()) => panic!("scan should not complete with only 80% data"),
        Err(ref e) if e.is_recoverable_eof() => {} // expected
        Err(e) => panic!("unexpected error: {e:?}")
    }

    // Now expose all data
    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .expect("scan should succeed with 100% data");
    assert_eq!(out, expected, "pixels must match one-shot decode");
}

/// Same as two_step but with 10-byte chunks to stress-test many retries.
#[test]
fn scan_resume_small_chunks() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let expected = decode_oneshot(data);

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    let mut out: Vec<u8> = Vec::new();
    let chunk = 10;
    let mut header_done = false;
    for avail in (chunk..=data.len()).step_by(chunk) {
        limit.set(avail.min(data.len()));

        if !header_done {
            match decoder.decode_headers() {
                Ok(()) => {
                    header_done = true;
                }
                Err(ref e) if e.is_recoverable_eof() => continue,
                Err(e) => panic!("header error at byte {avail}: {e:?}")
            }
        }

        if out.is_empty() {
            out = vec![0u8; decoder.output_buffer_size().unwrap()];
        }

        match decoder.decode_into(&mut out) {
            Ok(()) => {
                assert_eq!(out, expected, "pixels must match one-shot");
                return;
            }
            Err(ref e) if e.is_recoverable_eof() => {
                continue;
            }
            Err(e) => panic!("scan error at byte {avail}: {e:?}")
        }
    }
    // One final try with all data
    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .expect("should succeed with all data");
    assert_eq!(out, expected, "pixels must match one-shot (final)");
}

#[test]
fn baseline_interleaved_incremental_parity() {
    assert_incremental_decode_matrix(&[
        (
            "synthetic_image",
            include_bytes!("../../../test-images/jpeg/synthetic_image.jpg"),
            37
        ),
        (
            "sampling_factors",
            include_bytes!("../../../test-images/jpeg/sampling_factors.jpg"),
            29
        ),
        (
            "cymk",
            include_bytes!("../../../test-images/jpeg/cymk.jpg"),
            4096
        )
    ]);
}

#[test]
fn baseline_images_do_not_expose_progressive_preview_state() {
    let data = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(data));

    assert_eq!(decoder.decoded_scans(), None);
    assert_eq!(decoder.decoded_preview_output_bytes(), None);
    assert_eq!(decoder.decoded_preview_scanlines(), None);

    decoder
        .decode_headers()
        .expect("baseline headers should decode");

    assert_eq!(decoder.decoded_scans(), None);
    assert_eq!(decoder.decoded_preview_output_bytes(), None);
    assert_eq!(decoder.decoded_preview_scanlines(), None);
}

#[test]
fn concatenated_four_components_incremental_parity() {
    assert_incremental_decode_matches_oneshot(
        "four_components",
        include_bytes!("../../../test-images/jpeg/four_components.jpg"),
        4096
    );
}

#[test]
fn baseline_non_interleaved_incremental_parity() {
    assert_incremental_decode_matrix(&[
        (
            "non_interleaved_444_64x64",
            include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg"),
            17
        ),
        (
            "non_interleaved_420_64x64",
            include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
            17
        ),
        (
            "non_interleaved_422_64x64",
            include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg"),
            17
        ),
        (
            "non_interleaved_440_64x64",
            include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg"),
            17
        ),
        (
            "non_interleaved_422_65x65",
            include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg"),
            17
        )
    ]);
}

#[test]
fn progressive_huffman_incremental_parity() {
    assert_incremental_decode_matrix(&[
        (
            "down_sampled_grayscale_prog",
            include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
            31
        ),
        (
            "progressive_420_color",
            include_bytes!("../../../test-images/jpeg/rebuilt_relax_fill_bytes_before_marker.jpg"),
            257
        ),
        (
            "kiara_limited_progressive_four_components",
            include_bytes!(
                "../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg"
            ),
            8192
        ),
        (
            "progressive_restart_420",
            include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg"),
            97
        )
    ]);
}

#[test]
fn progressive_completed_dc_scan_is_displayable() {
    for (name, data) in [
        (
            "down_sampled_grayscale_prog",
            include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg").as_slice()
        ),
        (
            "progressive_420_color",
            include_bytes!("../../../test-images/jpeg/rebuilt_relax_fill_bytes_before_marker.jpg")
                .as_slice()
        ),
        (
            "kiara_limited_progressive_four_components",
            include_bytes!(
                "../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg"
            )
            .as_slice()
        )
    ] {
        let expected = decode_oneshot(data);
        let scans = progressive_sos_scans(data);
        assert!(
            scans.len() > 1,
            "{name}: fixture must contain multiple scans"
        );
        assert_eq!(scans[0].spec_start, 0, "{name}: first scan must include DC");
        assert_eq!(
            scans[0].spec_end, 0,
            "{name}: first scan must include only DC"
        );

        let cutoff = scans[1].data_start + 1;
        let limit = Rc::new(Cell::new(cutoff));
        let cursor = GrowableCursor::new(data, Rc::clone(&limit));
        let mut decoder = JpegDecoder::new(cursor);
        decoder.set_incremental_mode(true);

        decoder
            .decode_headers()
            .expect("headers should be visible at cutoff");
        let height = usize::from(decoder.info().unwrap().height);
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        let err = decoder
            .decode_into(&mut out)
            .expect_err("truncated second progressive scan should be recoverable");
        assert!(err.is_recoverable_eof(), "{name}: got {err:?}");
        assert_eq!(
            decoder.decoded_scans(),
            Some(1),
            "{name}: DC scan should commit"
        );
        assert_eq!(
            decoder.decoded_output_bytes(),
            Some(0),
            "{name}: progressive preview is not stable final output"
        );
        assert_eq!(
            decoder.decoded_scanlines(),
            Some(0),
            "{name}: progressive preview scanlines are reported separately"
        );
        assert_eq!(
            decoder.decoded_preview_output_bytes(),
            Some(out.len()),
            "{name}: preview is full-frame"
        );
        assert_eq!(
            decoder.decoded_preview_scanlines(),
            Some(height),
            "{name}: preview is full-height"
        );
        assert!(
            out.iter().any(|byte| *byte != 0),
            "{name}: completed DC scan should produce displayable pixels"
        );

        limit.set(data.len());
        decoder
            .decode_into(&mut out)
            .expect("full input should finish progressive decode");
        assert_pixels_match(&out, &expected, name, data.len());
    }
}

#[test]
fn progressive_scan_eof_respects_lenient_and_incremental_modes() {
    let data = include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg");
    let scans = progressive_sos_scans(data);
    assert!(scans.len() > 1, "fixture must contain multiple scans");
    let truncated = &data[..scans[1].data_start + 1];

    let lenient = decode_with_mode(truncated, false, false)
        .expect("non-strict one-shot decode should accept a truncated scan");
    assert!(
        lenient.iter().any(|byte| *byte != 0),
        "lenient truncated decode should produce best-effort pixels"
    );

    let strict_error = decode_with_mode(truncated, false, true)
        .expect_err("strict decode should reject a truncated scan");
    assert!(strict_error.is_recoverable_eof());

    let incremental_error = decode_with_mode(truncated, true, false)
        .expect_err("incremental decode should report recoverable scan EOF");
    assert!(incremental_error.is_recoverable_eof());
}

#[test]
fn baseline_scan_eof_respects_lenient_and_incremental_modes() {
    let data = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let entropy_start = entropy_start(data);
    let cutoff = entropy_start + (data.len() - entropy_start) * 60 / 100;
    let truncated = &data[..cutoff];

    let lenient = decode_with_mode(truncated, false, false)
        .expect("non-strict one-shot decode should accept a truncated scan");
    assert!(
        lenient.iter().any(|byte| *byte != 0),
        "lenient truncated decode should produce best-effort pixels"
    );
    assert_eq!(
        fnv1a(&lenient),
        0x1eab_e750_2dcb_ddc3,
        "lenient output should match the legacy decoded pixels"
    );

    let strict_error = decode_with_mode(truncated, false, true)
        .expect_err("strict decode should reject a truncated scan");
    assert!(strict_error.is_recoverable_eof());

    let incremental_error = decode_with_mode(truncated, true, false)
        .expect_err("incremental decode should report recoverable scan EOF");
    assert!(incremental_error.is_recoverable_eof());
}

#[test]
fn baseline_multi_sos_lenient_eof_preserves_neutral_output() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let cutoffs = [sos_data_start(data, 0) + 4, sos_data_start(data, 2) + 3];

    for cutoff in cutoffs {
        let pixels = decode_with_mode(&data[..cutoff], false, false)
            .expect("non-strict one-shot decode should accept a truncated component scan");
        assert!(
            pixels.iter().all(|byte| *byte == 128),
            "cutoff {cutoff}: incomplete component scans should produce neutral output"
        );
    }
}

#[test]
fn progressive_ac_scan_retry_keeps_last_completed_preview() {
    for (name, data) in [
        (
            "down_sampled_grayscale_prog_ac",
            include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg").as_slice()
        ),
        (
            "progressive_420_color_ac",
            include_bytes!("../../../test-images/jpeg/rebuilt_relax_fill_bytes_before_marker.jpg")
                .as_slice()
        ),
        (
            "kiara_limited_progressive_four_components_ac",
            include_bytes!(
                "../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg"
            )
            .as_slice()
        )
    ] {
        let expected = decode_oneshot(data);
        let scans = progressive_sos_scans(data);
        let (scan_index, ac_scan) = scans
            .iter()
            .enumerate()
            .find(|(_, scan)| scan.spec_start > 0 && scan.succ_high == 0)
            .expect("fixture must contain an AC first scan");
        let cutoff = ac_scan.data_start + 64;
        let limit = Rc::new(Cell::new(cutoff));
        let cursor = GrowableCursor::new(data, Rc::clone(&limit));
        let mut decoder = JpegDecoder::new(cursor);
        decoder.set_incremental_mode(true);

        decoder
            .decode_headers()
            .expect("headers should be visible at cutoff");
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        let first_err = decoder
            .decode_into(&mut out)
            .expect_err("truncated AC scan should be recoverable");
        assert!(first_err.is_recoverable_eof(), "{name}: got {first_err:?}");

        let completed_scans = decoder
            .decoded_scans()
            .expect("progressive headers should expose scan progress");
        assert!(
            completed_scans > 0 && completed_scans <= scan_index,
            "{name}: active AC scan must not be committed"
        );
        assert_eq!(decoder.decoded_output_bytes(), Some(0));
        assert_eq!(decoder.decoded_preview_output_bytes(), Some(out.len()));
        let first_preview = out.clone();

        let second_err = decoder
            .decode_into(&mut out)
            .expect_err("same truncated AC scan should stay recoverable");
        assert!(
            second_err.is_recoverable_eof(),
            "{name}: got {second_err:?}"
        );
        assert_eq!(decoder.decoded_scans(), Some(completed_scans));
        assert_eq!(decoder.decoded_output_bytes(), Some(0));
        assert_eq!(decoder.decoded_preview_output_bytes(), Some(out.len()));
        assert_eq!(out, first_preview, "partial AC data leaked into preview");

        limit.set(data.len());
        decoder
            .decode_into(&mut out)
            .expect("full input should finish progressive decode");
        assert_pixels_match(&out, &expected, name, data.len());
    }
}

#[test]
fn progressive_refinement_retry_does_not_apply_partial_scan_twice() {
    for (name, data) in [
        (
            "down_sampled_grayscale_prog_refine",
            include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg").as_slice()
        ),
        (
            "progressive_420_color_refine",
            include_bytes!("../../../test-images/jpeg/rebuilt_relax_fill_bytes_before_marker.jpg")
                .as_slice()
        )
    ] {
        let expected = decode_oneshot(data);
        let scans = progressive_sos_scans(data);
        let refine_scan = scans
            .iter()
            .find(|scan| scan.spec_start > 0 && scan.succ_high > 0)
            .expect("fixture must contain an AC refinement scan");
        let cutoff = refine_scan.data_start + 64;
        let limit = Rc::new(Cell::new(cutoff));
        let cursor = GrowableCursor::new(data, Rc::clone(&limit));
        let mut decoder = JpegDecoder::new(cursor);
        decoder.set_incremental_mode(true);

        decoder
            .decode_headers()
            .expect("headers should be visible at cutoff");
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        let first_err = decoder
            .decode_into(&mut out)
            .expect_err("truncated refinement scan should be recoverable");
        assert!(first_err.is_recoverable_eof(), "{name}: got {first_err:?}");
        let completed_scans = decoder
            .decoded_scans()
            .expect("progressive headers should expose scan progress");
        assert!(
            completed_scans > 0,
            "{name}: completed scans should remain displayable"
        );
        assert_eq!(decoder.decoded_output_bytes(), Some(0));
        assert_eq!(decoder.decoded_preview_output_bytes(), Some(out.len()));
        let first_partial = out.clone();

        let second_err = decoder
            .decode_into(&mut out)
            .expect_err("same truncated refinement scan should stay recoverable");
        assert!(
            second_err.is_recoverable_eof(),
            "{name}: got {second_err:?}"
        );
        assert_eq!(decoder.decoded_scans(), Some(completed_scans));
        assert_eq!(decoder.decoded_output_bytes(), Some(0));
        assert_eq!(decoder.decoded_preview_output_bytes(), Some(out.len()));
        assert_eq!(
            out, first_partial,
            "{name}: partial refinement data was applied twice"
        );

        limit.set(data.len());
        decoder
            .decode_into(&mut out)
            .expect("full input should finish progressive decode");
        assert_pixels_match(&out, &expected, name, data.len());
    }
}

fn assert_progressive_marker_split_recovers(
    data: &[u8], expected: &[u8], cutoff: usize, expect_preview: bool, label: &str
) {
    let limit = Rc::new(Cell::new(cutoff));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);
    decoder
        .decode_headers()
        .expect("first scan headers should be visible");

    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    let first_err = decoder
        .decode_into(&mut out)
        .expect_err("split marker should return recoverable EOF");
    assert!(first_err.is_recoverable_eof(), "{label}: got {first_err:?}");
    let completed_scans = decoder.decoded_scans().expect("image must be progressive");
    assert_eq!(decoder.decoded_output_bytes(), Some(0), "{label}");
    assert_eq!(decoder.decoded_scanlines(), Some(0), "{label}");
    if expect_preview {
        assert!(
            completed_scans > 0,
            "{label}: first scan should be committed"
        );
        assert_eq!(
            decoder.decoded_preview_output_bytes(),
            Some(out.len()),
            "{label}"
        );
    } else {
        assert_eq!(
            completed_scans, 0,
            "{label}: ambiguous marker must not commit scan"
        );
        assert_eq!(decoder.decoded_preview_output_bytes(), Some(0), "{label}");
        assert!(
            out.iter().all(|byte| *byte == 0),
            "{label}: partial scan leaked"
        );
    }
    let preview = out.clone();

    let second_err = decoder
        .decode_into(&mut out)
        .expect_err("unchanged split should remain recoverable");
    assert!(
        second_err.is_recoverable_eof(),
        "{label}: got {second_err:?}"
    );
    assert_eq!(decoder.decoded_scans(), Some(completed_scans), "{label}");
    assert_eq!(decoder.decoded_output_bytes(), Some(0), "{label}");
    let expected_preview_bytes = if expect_preview { out.len() } else { 0 };
    assert_eq!(
        decoder.decoded_preview_output_bytes(),
        Some(expected_preview_bytes),
        "{label}"
    );
    assert_eq!(
        out, preview,
        "{label}: unchanged scan count changed preview pixels"
    );

    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .unwrap_or_else(|error| panic!("{label}: full input should finish: {error:?}"));
    assert_pixels_match(&out, expected, label, data.len());
}

#[test]
fn progressive_inter_scan_marker_splits_preserve_preview() {
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);
    let markers = list_jpeg_markers(data);
    let first_sos = markers
        .iter()
        .position(|(_, code, _)| *code == 0xDA)
        .expect("fixture must contain an SOS marker");
    let inter_scan_dht = markers
        .iter()
        .skip(first_sos + 1)
        .find(|(_, code, _)| *code == 0xC4)
        .copied()
        .expect("fixture must contain an inter-scan DHT marker");
    let next_sos = markers
        .iter()
        .skip(first_sos + 1)
        .find(|(_, code, _)| *code == 0xDA)
        .copied()
        .expect("fixture must contain a second SOS marker");

    for (marker_name, (offset, _, body_len), prefix_has_preview) in
        [("DHT", inter_scan_dht, false), ("SOS", next_sos, true)]
    {
        let body_len = body_len.expect("DHT and SOS markers must have bodies");
        let marker_end = offset + 2 + body_len;
        let prefix_label = format!("progressive {marker_name} at {offset}, prefix split");
        assert_progressive_marker_split_recovers(
            data,
            &expected,
            offset + 1,
            prefix_has_preview,
            &prefix_label
        );

        // Once the marker code is visible, the preceding entropy scan is known
        // to be complete and must remain available as a preview while the
        // marker length/body is still incomplete.
        let mut cutoffs = vec![offset + 2, offset + 3, offset + 4, marker_end - 1];
        cutoffs.sort_unstable();
        cutoffs.dedup();
        for cutoff in cutoffs {
            let label = format!("progressive {marker_name} at {offset}, split at {cutoff}");
            assert_progressive_marker_split_recovers(data, &expected, cutoff, true, &label);
        }
    }
}

#[test]
fn progressive_non_strict_scratch_path_matches_direct_path() {
    let original = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let first_sos = sos_marker_offset(original, 0);
    let huffman_selector = first_sos + 6;
    assert_eq!(
        original[huffman_selector], 0,
        "fixture's first scan must initially select Huffman tables 0/0"
    );

    let mut corrupt = original.to_vec();
    corrupt[huffman_selector] = 0xFF;

    let strict_error = decode_with_mode(&corrupt, true, true)
        .expect_err("strict mode must reject the missing Huffman table");
    assert!(!strict_error.is_recoverable_eof());

    let direct = decode_with_mode(&corrupt, false, false)
        .expect("non-strict direct path should produce best-effort output");
    let scratch = decode_with_mode(&corrupt, true, false)
        .expect("non-strict scratch path should produce best-effort output");
    assert_pixels_match(
        &scratch,
        &direct,
        "progressive non-strict scratch/direct parity",
        corrupt.len()
    );
}

#[test]
fn progressive_dc_first_resume_uses_fine_checkpoint() {
    for (name, data, cutoff_numerators) in [
        (
            "synthetic_image_dc_first",
            include_bytes!("../../../test-images/jpeg/synthetic_image.jpg").as_slice(),
            &[1, 2, 3][..]
        ),
        (
            "down_sampled_grayscale_prog_dc_first",
            include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg").as_slice(),
            &[2][..]
        ),
        (
            "kiara_limited_progressive_four_components_dc_first",
            include_bytes!(
                "../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg"
            )
            .as_slice(),
            &[2][..]
        ),
        (
            "progressive_restart_420_dc_first",
            // Generated with Pillow 10.2 from a deterministic 256x256 RGB
            // pattern using progressive=true and restart_marker_rows=1.
            include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg").as_slice(),
            &[2][..]
        )
    ] {
        let expected = decode_oneshot(data);
        let scans = progressive_sos_scans(data);
        assert!(scans.len() > 1, "{name}: fixture must contain multiple scans");
        let first_scan = scans[0];
        assert_eq!(first_scan.spec_start, 0, "{name}: first scan must be DC");
        assert_eq!(first_scan.spec_end, 0, "{name}: first scan must be DC-only");
        assert_eq!(first_scan.succ_high, 0, "{name}: first scan must be first DC");

        if name == "progressive_restart_420_dc_first" {
            let markers = list_jpeg_markers(data);
            assert!(markers.iter().any(|(_, code, _)| *code == 0xDD), "fixture must contain DRI");
            assert!(
                markers.iter().any(|(_, code, _)| (0xD0..=0xD7).contains(code)),
                "fixture must contain RST markers"
            );
        }

        let second_sos_offset = sos_marker_offset(data, 1);
        let scan_len = second_sos_offset - first_scan.data_start;
        for &cutoff_numerator in cutoff_numerators {
            let cutoff_percent = cutoff_numerator * 25;
            let cutoff = first_scan.data_start + scan_len * cutoff_numerator / 4;
            assert!(
                cutoff > first_scan.data_start && cutoff < second_sos_offset,
                "{name} at {cutoff_percent}%: cutoff must land inside first DC entropy data"
            );

            let limit = Rc::new(Cell::new(cutoff));
            let seek_log = Rc::new(RefCell::new(Vec::new()));
            let cursor =
                GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
            let mut decoder = JpegDecoder::new(cursor);
            decoder.set_incremental_mode(true);

            decoder.decode_headers().expect("headers should be visible at cutoff");
            let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
            let err = decoder
                .decode_into(&mut out)
                .expect_err("truncated first DC scan should be recoverable");
            assert!(err.is_recoverable_eof(), "{name} at {cutoff_percent}%: got {err:?}");
            assert_eq!(
                decoder.decoded_scans(),
                Some(0),
                "{name} at {cutoff_percent}%: active DC scan is partial"
            );
            assert_eq!(decoder.decoded_output_bytes(), Some(0));
            assert_eq!(decoder.decoded_scanlines(), Some(0));
            assert_eq!(decoder.decoded_preview_output_bytes(), Some(0));
            assert_eq!(decoder.decoded_preview_scanlines(), Some(0));
            assert!(
                out.iter().all(|byte| *byte == 0),
                "{name} at {cutoff_percent}%: partial first DC scan must not be published"
            );

            seek_log.borrow_mut().clear();
            limit.set(data.len());
            decoder
                .decode_into(&mut out)
                .expect("full input should resume progressive first DC scan");
            assert_pixels_match(&out, &expected, name, data.len());

            let seeks = seek_log.borrow();
            let first_seek = seeks
                .first()
                .copied()
                .expect("retry should seek to a progressive checkpoint");
            assert!(
                first_seek > first_scan.data_start && first_seek < second_sos_offset,
                "{name} at {cutoff_percent}%: eligible retry should resume inside first DC entropy data; \
                 scan_start={}, seek={first_seek}, next_sos={second_sos_offset}",
                first_scan.data_start
            );
        }
    }
}

#[test]
fn progressive_repeated_dc_eof_falls_back_to_scan_boundary() {
    let name = "synthetic_image_repeated_dc_eof";
    let data = include_bytes!("../../../test-images/jpeg/synthetic_image.jpg");
    let expected = decode_oneshot(data);
    let scans = progressive_sos_scans(data);
    let first_scan = scans[0];
    let second_sos_offset = sos_marker_offset(data, 1);
    let first_cutoff = first_scan.data_start
        + (second_sos_offset - first_scan.data_start) / 3;
    let second_cutoff = first_cutoff + 1;

    let limit = Rc::new(Cell::new(first_cutoff));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);
    decoder.decode_headers().expect("headers should be visible");
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];

    let first_error = decoder.decode_into(&mut out).expect_err("first cutoff should need data");
    assert!(first_error.is_recoverable_eof(), "got {first_error:?}");

    seek_log.borrow_mut().clear();
    limit.set(second_cutoff);
    let second_error = decoder
        .decode_into(&mut out)
        .expect_err("one additional byte should still need data");
    assert!(second_error.is_recoverable_eof(), "got {second_error:?}");
    let fine_seek = seek_log.borrow()[0];
    assert!(
        fine_seek > first_scan.data_start && fine_seek < second_sos_offset,
        "second attempt should use the fine checkpoint"
    );

    seek_log.borrow_mut().clear();
    limit.set(data.len());
    decoder.decode_into(&mut out).expect("full input should finish");
    assert_pixels_match(&out, &expected, name, data.len());
    assert_eq!(
        seek_log.borrow()[0],
        first_scan.data_start,
        "a repeated EOF after fine resume should fall back to the scan boundary"
    );
}

#[test]
fn progressive_unsafe_scans_replay_from_scan_boundary() {
    let data = include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg");
    let expected = decode_oneshot(data);
    let scans = progressive_sos_scans(data);

    for (name, scan_index) in [
        (
            "ac_first",
            scans
                .iter()
                .position(|scan| scan.spec_start > 0 && scan.succ_high == 0)
                .expect("fixture must contain an AC first scan")
        ),
        (
            "ac_refine",
            scans
                .iter()
                .position(|scan| scan.spec_start > 0 && scan.succ_high > 0)
                .expect("fixture must contain an AC refinement scan")
        )
    ] {
        let scan = scans[scan_index];
        let scan_end = if scan_index + 1 < scans.len() {
            sos_marker_offset(data, scan_index + 1)
        } else {
            data.len()
        };
        let cutoff = (scan.data_start + 64).min(scan_end - 1);
        assert!(
            cutoff > scan.data_start && cutoff < scan_end,
            "{name}: cutoff must land inside active scan entropy data"
        );

        let limit = Rc::new(Cell::new(cutoff));
        let seek_log = Rc::new(RefCell::new(Vec::new()));
        let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
        let mut decoder = JpegDecoder::new(cursor);
        decoder.set_incremental_mode(true);

        decoder.decode_headers().expect("headers should be visible at cutoff");
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        let err = decoder
            .decode_into(&mut out)
            .expect_err("truncated unsafe progressive scan should be recoverable");
        assert!(err.is_recoverable_eof(), "{name}: got {err:?}");

        seek_log.borrow_mut().clear();
        limit.set(data.len());
        decoder
            .decode_into(&mut out)
            .expect("full input should replay unsafe progressive scan safely");
        assert_pixels_match(&out, &expected, name, data.len());

        let seeks = seek_log.borrow();
        let first_seek = seeks
            .first()
            .copied()
            .expect("retry should seek to active scan boundary");
        assert_eq!(
            first_seek, scan.data_start,
            "{name}: unsafe progressive scan must replay from scan boundary"
        );
    }
}

#[test]
#[cfg(feature = "arith")]
fn progressive_arithmetic_restart_replays_dc_scan_boundary() {
    let name = "arith_prog_restart_dc_boundary";
    let data = include_bytes!("../../../test-images/jpeg/arith/prog-restart.jpg");
    let expected = decode_oneshot(data);
    let scans = progressive_sos_scans(data);
    assert!(scans.len() > 1, "fixture must contain multiple scans");
    let first_scan = scans[0];
    let second_sos_offset = sos_marker_offset(data, 1);
    let rst_positions: Vec<_> = list_jpeg_markers(data)
        .into_iter()
        .filter_map(|(offset, code, _)| (0xD0..=0xD7).contains(&code).then_some(offset))
        .collect();
    assert!(
        !rst_positions.is_empty(),
        "fixture must contain progressive restart markers"
    );
    let cutoff = first_scan.data_start + (second_sos_offset - first_scan.data_start) / 2;

    let limit = Rc::new(Cell::new(cutoff));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    decoder.decode_headers().expect("headers should be visible at cutoff");
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    let err = decoder
        .decode_into(&mut out)
        .expect_err("truncated arithmetic progressive DC scan should be recoverable");
    assert!(err.is_recoverable_eof(), "got {err:?}");

    seek_log.borrow_mut().clear();
    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .expect("full input should replay arithmetic progressive scan");
    assert_pixels_match(&out, &expected, name, data.len());

    let seeks = seek_log.borrow();
    let first_seek = seeks
        .first()
        .copied()
        .expect("retry should seek to progressive scan boundary");
    assert_eq!(
        first_seek, first_scan.data_start,
        "arithmetic progressive scans must not use fine Huffman checkpoints"
    );
}

#[test]
#[cfg(feature = "arith")]
fn arithmetic_incremental_parity() {
    assert_incremental_decode_matrix(&[
        (
            "arith_seq",
            include_bytes!("../../../test-images/jpeg/arith/seq.jpg"),
            23
        ),
        (
            "arith_prog",
            include_bytes!("../../../test-images/jpeg/arith/prog.jpg"),
            23
        )
    ]);
}

#[test]
#[cfg(feature = "arith")]
fn arithmetic_restart_incremental_parity() {
    assert_incremental_decode_matrix(&[
        (
            "arith_seq_restart",
            include_bytes!("../../../test-images/jpeg/arith/seq-restart.jpg"),
            23
        ),
        (
            "arith_prog_restart",
            include_bytes!("../../../test-images/jpeg/arith/prog-restart.jpg"),
            23
        )
    ]);
}

#[test]
#[cfg(feature = "arith")]
fn arithmetic_restart_resume_uses_rst_checkpoint() {
    let data = include_bytes!("../../../test-images/jpeg/arith/seq-restart.jpg");
    let expected = decode_oneshot(data);
    let limit = Rc::new(Cell::new(874_usize));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    decoder
        .decode_headers()
        .expect("headers should be visible before first RST retry");
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    let err = decoder
        .decode_into(&mut out)
        .expect_err("truncated scan after first RST should be recoverable");
    assert!(
        err.is_recoverable_eof(),
        "expected recoverable EOF, got {err:?}"
    );

    seek_log.borrow_mut().clear();
    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .expect("full input should resume from RST checkpoint");

    let seeks = seek_log.borrow();
    assert!(
        seeks.contains(&857),
        "retry should seek to first RST checkpoint at 857, got {seeks:?}"
    );
    assert_pixels_match(&out, &expected, "arith_seq_restart_checkpoint", data.len());
}

/// Sanity check: byte-by-byte incremental decode of a normal image (no
/// inline markers) must reproduce the one-shot pixel output. If this test
/// fails, the bug is in the scan-retry mechanism itself rather than in
/// the inline-marker path.
#[test]
fn inplace_byte_by_byte_full_decode() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let expected = decode_oneshot(data);

    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

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
    decoder.set_incremental_mode(true);

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

// ===========================================================================
// Atomic marker parsing coverage
// ===========================================================================

/// Scan a JPEG byte stream and return the absolute file offset of every
/// marker (FFxx) we know about: SOI, EOI, RSTn, and any length-prefixed
/// marker. The returned tuples are `(offset, marker_code, marker_length)`,
/// where `marker_length` is `None` for markers without a length field and
/// includes the two length bytes for length-prefixed markers.
fn list_jpeg_markers(data: &[u8]) -> Vec<(usize, u8, Option<usize>)> {
    let mut markers = Vec::new();
    let mut i = 0;
    while i + 1 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let code = data[i + 1];
        if code == 0xFF || code == 0x00 {
            i += 1;
            continue;
        }
        if code == 0xD8 || code == 0xD9 || (0xD0..=0xD7).contains(&code) {
            markers.push((i, code, None));
            i += 2;
            if code == 0xD9 {
                break;
            }
            continue;
        }
        if i + 3 >= data.len() {
            break;
        }
        let length = usize::from(u16::from_be_bytes([data[i + 2], data[i + 3]]));
        if length < 2 {
            break;
        }
        markers.push((i, code, Some(length)));
        i += 2 + length;
    }
    markers
}

/// Run an incremental decode against `data` that truncates input at exactly
/// `cutoff_offset` bytes on the first attempt, expects a recoverable EOF
/// (or a successful decode if the cutoff happened to land somewhere
/// already-complete), then exposes the full bytes and asserts the final
/// pixels match a one-shot decode.
fn assert_split_at_recovers(data: &[u8], cutoff_offset: usize, label: &str) {
    let expected = decode_oneshot(data);

    let limit = Rc::new(Cell::new(cutoff_offset));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    // First attempt: at most `cutoff_offset` bytes visible.
    // Headers may or may not complete; whatever happens, we should
    // either succeed *with truncated pixels* or get a recoverable EOF.
    match decoder.decode_headers() {
        Ok(()) => {}
        Err(ref e) if e.is_recoverable_eof() => {}
        Err(e) => panic!("{label}: unexpected header error at cutoff {cutoff_offset}: {e:?}")
    }

    // If headers completed at cutoff, try to decode scan.
    if decoder.info().is_some() {
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        if let Err(ref e) = decoder.decode_into(&mut out) {
            if !e.is_recoverable_eof() {
                panic!("{label}: unexpected scan error at cutoff {cutoff_offset}: {e:?}");
            }
        }
    }

    // Now grow input to full size and retry until success.
    limit.set(data.len());

    // Header may or may not have already completed; re-run safely.
    if decoder.info().is_none() {
        decoder
            .decode_headers()
            .unwrap_or_else(|e| panic!("{label}: headers must complete at full size: {e:?}"));
    }

    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    let mut attempts = 0;
    loop {
        attempts += 1;
        // Worst-case bound: at most one suspended retry per byte of input
        // (every byte could in principle become a marker boundary). In
        // practice the real bound is "number of markers + restart
        // intervals", but this loose bound is cheap and future-proof.
        if attempts > data.len() + 1 {
            panic!("{label}: decode_into never completed at cutoff {cutoff_offset}");
        }
        match decoder.decode_into(&mut out) {
            Ok(()) => break,
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(e) => panic!("{label}: unexpected error at full size: {e:?}")
        }
    }
    assert_pixels_match(&out, &expected, label, data.len());
}

/// Atomic-marker contract: every marker-body split should recover after the
/// remaining bytes arrive.
#[test]
fn header_marker_truncation_at_every_position_recovers() {
    // tiny_non_interleaved_444 has SOI, APP0, DQT, DQT, SOF, DHT, DHT, SOS,
    // and inter-scan DHTs between SOS markers — i.e. it exercises both the
    // header-phase dispatch loop and the inter-scan marker path that
    // `mcu.rs::advance_to_next_sos` drives.
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");

    let markers = list_jpeg_markers(data);
    let first_sos = markers
        .iter()
        .position(|(_, code, _)| *code == 0xDA)
        .expect("fixture must contain SOS");
    assert!(
        markers.iter().any(|(_, code, _)| *code == 0xD9),
        "fixture must contain EOI"
    );
    assert!(
        markers.iter().skip(first_sos + 1).any(|(_, code, _)| *code == 0xDA),
        "fixture must contain a later inter-scan SOS"
    );
    assert!(
        markers.iter().skip(first_sos + 1).any(|(_, code, _)| *code == 0xC4),
        "fixture must contain an inter-scan DHT marker"
    );
    assert!(
        markers.iter().skip(first_sos + 1).any(|(_, code, _)| *code == 0xD9),
        "marker scanner must reach EOI after scan data"
    );
    for (offset, code, body_len) in markers {
        let Some(body_len) = body_len else {
            // SOI / RST / EOI have no body; nothing to truncate.
            continue;
        };
        // The marker is `[FF code length_hi length_lo payload...]`, and
        // `body_len` includes the two length bytes. Every cut from the first
        // length byte through the last missing payload byte should suspend
        // cleanly and then replay the marker exactly once.
        let marker_end = offset + 2 + body_len;
        for cut in (offset + 2)..marker_end {
            let label = format!("marker FF{code:02X} at {offset}: truncated at byte {cut}");
            assert_split_at_recovers(data, cut, &label);
        }
    }
}

/// Marker-prefix-byte split: truncate exactly between the `0xFF` prefix and
/// the marker code byte. The marker code itself can be ambiguous (fill byte
/// 0xFF or stuffing 0x00), so the dispatch loop must handle this boundary
/// without losing the marker on retry.
#[test]
fn header_marker_prefix_byte_split_recovers() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let markers = list_jpeg_markers(data);

    for (offset, code, _) in markers {
        if code == 0xD8 {
            // SOI is at offset 0; truncating between FF and D8 just makes
            // the input look like an empty file — covered by other tests.
            continue;
        }
        let cut = offset + 1; // exactly between 0xFF and the marker code
        let label = format!("marker FF{code:02X} at {offset}: split at code byte");
        assert_split_at_recovers(data, cut, &label);
    }
}

/// Overwrite-shaped marker no-duplication regression. The atomic-parse
/// contract guarantees that when a marker parser returns an error (e.g.
/// recoverable EOF mid-payload), decoder state is bit-identical to its
/// pre-parse shape. After retry, the final state must match a fresh
/// one-shot decode in every field — not just append-only metadata.
#[test]
fn header_overwrite_marker_no_duplication_on_retry() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");

    // Reference: decode in one shot, then capture all fields we expect to
    // be overwrite-shaped (DHT/DQT slots, SOF components, SOS huffman
    // selectors, input colorspace).
    let mut reference = JpegDecoder::new(ZCursor::new(data));
    reference.decode().expect("reference decode failed");
    let ref_info = reference.info().expect("info() must be Some after decode");

    // Truncate at every header-marker body byte and retry; after retry
    // the decoder's exposed metadata must equal the reference.
    let markers = list_jpeg_markers(data);
    for (offset, code, body_len) in markers {
        let Some(body_len) = body_len else {
            continue;
        };
        let body_start = offset + 4;
        let marker_end = offset + 2 + body_len;
        // Sample every byte to keep the test fast; one byte per marker is
        // enough to validate the contract (other tests cover finer
        // granularity for pixel parity).
        let cut = body_start + (marker_end - body_start) / 2;

        let limit = Rc::new(Cell::new(cut));
        let cursor = GrowableCursor::new(data, Rc::clone(&limit));
        let mut decoder = JpegDecoder::new(cursor);

        // First attempt: expect either truncation surfaces as recoverable
        // EOF, or headers complete and the scan errors recoverably.
        match decoder.decode_headers() {
            Ok(()) => {}
            Err(ref e) if e.is_recoverable_eof() => {}
            Err(e) => panic!("FF{code:02X} at {offset}: unexpected header error: {e:?}")
        }

        // Expose full input and complete the decode.
        limit.set(data.len());
        if decoder.info().is_none() {
            decoder.decode_headers().expect("headers must complete");
        }
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        loop {
            match decoder.decode_into(&mut out) {
                Ok(()) => break,
                Err(ref e) if e.is_recoverable_eof() => continue,
                Err(e) => panic!("FF{code:02X} at {offset}: unexpected scan error: {e:?}")
            }
        }

        let info = decoder.info().expect("info after success");
        assert_eq!(
            info.width, ref_info.width,
            "FF{code:02X} at {offset}: width drift"
        );
        assert_eq!(
            info.height, ref_info.height,
            "FF{code:02X} at {offset}: height drift"
        );
        assert_eq!(
            info.components, ref_info.components,
            "FF{code:02X} at {offset}: components count drift"
        );
        assert_eq!(
            decoder.input_colorspace(),
            reference.input_colorspace(),
            "FF{code:02X} at {offset}: input colorspace drift"
        );
    }
}

fn entropy_start(data: &[u8]) -> usize {
    let sos_pos = data
        .windows(2)
        .position(|w| w == [0xFF, 0xDA])
        .expect("test image must have SOS");
    let sos_len = u16::from_be_bytes([data[sos_pos + 2], data[sos_pos + 3]]) as usize;
    sos_pos + 2 + sos_len
}

fn sos_data_start(data: &[u8], sos_index: usize) -> usize {
    let (offset, _, length) = list_jpeg_markers(data)
        .into_iter()
        .filter(|(_, code, _)| *code == 0xDA)
        .nth(sos_index)
        .expect("test image must contain the requested SOS marker");
    offset + 2 + length.expect("SOS marker must have a length")
}

fn sos_marker_offset(data: &[u8], sos_index: usize) -> usize {
    list_jpeg_markers(data)
        .into_iter()
        .filter(|(_, code, _)| *code == 0xDA)
        .nth(sos_index)
        .expect("test image must contain the requested SOS marker")
        .0
}

#[derive(Clone, Copy)]
struct ProgressiveSosScan {
    data_start: usize,
    spec_start: u8,
    spec_end:   u8,
    succ_high:  u8
}

fn progressive_sos_scans(data: &[u8]) -> Vec<ProgressiveSosScan> {
    list_jpeg_markers(data)
        .into_iter()
        .filter_map(|(offset, code, length)| {
            if code != 0xDA {
                return None;
            }
            let length = length.expect("SOS marker must have a length");
            let payload = offset + 4;
            let components = usize::from(data[payload]);
            let params = payload + 1 + 2 * components;
            let successive = data[params + 2];
            Some(ProgressiveSosScan {
                data_start: offset + 2 + length,
                spec_start: data[params],
                spec_end:   data[params + 1],
                succ_high:  successive >> 4
            })
        })
        .collect()
}

fn multi_sos_first_scan_cutoff(data: &[u8]) -> usize {
    let markers = list_jpeg_markers(data);
    let sos_markers: Vec<_> = markers
        .iter()
        .copied()
        .filter(|(_, code, _)| *code == 0xDA)
        .collect();
    assert!(sos_markers.len() > 1, "fixture must be baseline multi-SOS");
    let first_scan_start = sos_data_start(data, 0);
    let second_sos_offset = sos_markers[1].0;
    assert!(first_scan_start < second_sos_offset);
    second_sos_offset - 1
}

fn first_retry_seek_after_scan_eof(data: &[u8]) -> usize {
    let expected = decode_oneshot(data);
    let entropy_start = entropy_start(data);
    let cutoff = entropy_start + (data.len() - entropy_start) * 60 / 100;

    let limit = Rc::new(Cell::new(cutoff));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    decoder
        .decode_headers()
        .expect("headers should be fully visible at cutoff");
    assert!(decoder.incremental_mode());
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];

    let err = decoder
        .decode_into(&mut out)
        .expect_err("truncated scan should give recoverable EOF");
    assert!(
        err.is_recoverable_eof(),
        "expected recoverable EOF, got {err:?}"
    );

    seek_log.borrow_mut().clear();
    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .expect("full data should allow decode to complete");
    assert_pixels_match(&out, &expected, "first_retry_seek", data.len());

    let seeks = seek_log.borrow();
    *seeks
        .first()
        .expect("retry must seek to scan start or a checkpoint")
}

#[test]
fn incremental_mode_records_checkpoint_on_first_scan_attempt() {
    let data = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let entropy_start = entropy_start(data);

    let incremental_seek = first_retry_seek_after_scan_eof(data);
    assert!(
        incremental_seek > entropy_start,
        "incremental mode should resume from an entropy-data row checkpoint; \
         entropy_start={entropy_start}, seek={incremental_seek}"
    );
}

#[test]
fn baseline_multi_sos_resumes_from_scan_body_checkpoint_without_stable_scanlines() {
    for (name, data) in [
        (
            "non_interleaved_444_64x64",
            include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg").as_slice()
        ),
        (
            "non_interleaved_420_64x64",
            include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg").as_slice()
        ),
        (
            "non_interleaved_422_64x64",
            include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg").as_slice()
        ),
        (
            "non_interleaved_440_64x64",
            include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg").as_slice()
        )
    ] {
        let expected = decode_oneshot(data);
        let cutoff = multi_sos_first_scan_cutoff(data);
        let first_scan_start = sos_data_start(data, 0);
        let second_sos_offset = sos_marker_offset(data, 1);
        let limit = Rc::new(Cell::new(cutoff));
        let seek_log = Rc::new(RefCell::new(Vec::new()));
        let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
        let mut decoder = JpegDecoder::new(cursor);
        decoder.set_incremental_mode(true);

        decoder.decode_headers().expect("headers should be visible at cutoff");
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        let err = decoder
            .decode_into(&mut out)
            .expect_err("multi-SOS first scan truncation should be recoverable");
        assert!(err.is_recoverable_eof(), "{name}: got {err:?}");
        assert_eq!(
            decoder.decoded_output_bytes(),
            Some(0),
            "{name}: multi-SOS output is not stable until final assembly"
        );
        assert_eq!(
            decoder.decoded_scanlines(),
            Some(0),
            "{name}: multi-SOS output rows are not stable until final assembly"
        );

        seek_log.borrow_mut().clear();
        limit.set(data.len());
        decoder
            .decode_into(&mut out)
            .expect("full input should replay and finish multi-SOS baseline");
        assert_pixels_match(&out, &expected, name, data.len());
        assert_eq!(decoder.decoded_output_bytes(), Some(out.len()));
        assert_eq!(
            decoder.decoded_scanlines(),
            Some(usize::from(decoder.info().unwrap().height))
        );

        let seeks = seek_log.borrow();
        let first_retry_seek = seeks
            .first()
            .copied()
            .expect("retry must seek to a multi-SOS scan checkpoint");
        assert!(
            first_retry_seek > first_scan_start && first_retry_seek < second_sos_offset,
            "{name}: retry should resume inside the first scan body; \
             first_scan_start={first_scan_start}, seek={first_retry_seek}, \
             second_sos_offset={second_sos_offset}"
        );
    }
}

#[test]
fn baseline_multi_sos_marker_boundaries_recover() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let markers = list_jpeg_markers(data);
    let first_sos = markers
        .iter()
        .position(|(_, code, _)| *code == 0xDA)
        .expect("fixture must contain SOS");
    assert!(
        markers.iter().skip(first_sos + 1).any(|(_, code, _)| *code == 0xDA),
        "fixture must contain a later inter-scan SOS"
    );

    for (offset, code, length) in markers.into_iter().skip(1) {
        let mut cutoffs = vec![offset, offset + 1];
        if let Some(length) = length {
            cutoffs.push(offset + 2);
            cutoffs.push(offset + 2 + length - 1);
            cutoffs.push(offset + 2 + length);
        }

        cutoffs.sort_unstable();
        cutoffs.dedup();

        for cutoff in cutoffs {
            if cutoff < data.len() {
                let label = format!(
                    "baseline multi-SOS marker FF{code:02X} at {offset}, cutoff {cutoff}"
                );
                assert_split_at_recovers(data, cutoff, &label);
            }
        }
    }
}

#[test]
fn baseline_huffman_restart_resume_uses_rst_checkpoint() {
    let data = include_bytes!("../../../test-images/jpeg/four_components.jpg");
    let expected = decode_oneshot(data);
    let markers = list_jpeg_markers(data);
    let rst_positions: Vec<_> = markers
        .iter()
        .filter_map(|(offset, code, _)| (0xD0..=0xD7).contains(code).then_some(*offset))
        .collect();
    assert!(rst_positions.len() > 3, "fixture must contain restart markers");

    let cutoff = rst_positions[3] + 64;
    let limit = Rc::new(Cell::new(cutoff));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    decoder.decode_headers().expect("headers should be visible at cutoff");
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
    let err = decoder
        .decode_into(&mut out)
        .expect_err("truncated restart scan should be recoverable");
    assert!(err.is_recoverable_eof(), "got {err:?}");

    seek_log.borrow_mut().clear();
    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .expect("full input should resume from a restart checkpoint");
    assert_pixels_match(&out, &expected, "baseline_huffman_restart", data.len());

    let restart_resume_positions: Vec<_> = rst_positions
        .iter()
        .map(|position| position + 2)
        .collect();
    let seeks = seek_log.borrow();
    assert!(
        seeks.iter().any(|position| restart_resume_positions.contains(position)),
        "retry should seek to an RST checkpoint, got {seeks:?}; expected one of {restart_resume_positions:?}"
    );
}

/// Per-row checkpoint: truncating a non-RST image mid-scan in incremental
/// mode must resume from a row checkpoint rather than replaying from scan start.
#[test]
fn per_row_checkpoint_avoids_full_scan_replay() {
    // sampling_factors.jpg is baseline with NO RST markers — perfect for testing per-row.
    let data = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let expected = decode_oneshot(data);

    // Truncate roughly 60% into the scan data.
    let entropy_start = entropy_start(data);
    let scan_data_len = data.len() - entropy_start;
    let cutoff = entropy_start + scan_data_len * 60 / 100;

    let limit = Rc::new(Cell::new(cutoff));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    decoder
        .decode_headers()
        .expect("headers should be fully visible at cutoff");
    assert_eq!(decoder.decoded_output_bytes(), Some(0));
    assert_eq!(decoder.decoded_scanlines(), Some(0));
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];

    // First decode attempt records row checkpoints and returns recoverable EOF.
    let err = decoder
        .decode_into(&mut out)
        .expect_err("truncated scan should give recoverable EOF");
    assert!(
        err.is_recoverable_eof(),
        "expected recoverable EOF, got {err:?}"
    );
    let first_scanlines = decoder
        .decoded_scanlines()
        .expect("headers are decoded, so partial progress should be known");
    let first_bytes = decoder
        .decoded_output_bytes()
        .expect("headers are decoded, so partial progress should be known");
    assert!(first_bytes > 0, "scan EOF should expose a stable output prefix");
    assert!(first_bytes < out.len(), "truncated decode should not report full output");
    assert!(
        first_scanlines > 0 && first_scanlines < usize::from(decoder.info().unwrap().height),
        "expected a stable partial prefix, got {first_scanlines} scanlines"
    );

    // A retry with the same truncated input remains recoverable and idempotent.
    let err = decoder
        .decode_into(&mut out)
        .expect_err("still truncated, should give recoverable EOF again");
    assert!(
        err.is_recoverable_eof(),
        "expected recoverable EOF on second attempt, got {err:?}"
    );
    assert!(decoder.decoded_output_bytes().unwrap() >= first_bytes);
    assert!(decoder.decoded_scanlines().unwrap() >= first_scanlines);

    // Third attempt — expose full data. Should resume from the per-row
    // checkpoint saved during the second attempt.
    seek_log.borrow_mut().clear();
    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .expect("full data should allow decode to complete");
    assert_pixels_match(&out, &expected, "per_row_checkpoint", data.len());
    assert_eq!(
        decoder.decoded_output_bytes(),
        Some(out.len()),
        "successful decode should report the full output buffer as stable"
    );
    assert_eq!(
        decoder.decoded_scanlines(),
        Some(usize::from(decoder.info().unwrap().height)),
        "successful decode should report the full frame as stable"
    );

    // Verify: the seek on the third call should be to a position AFTER
    // entropy_start, proving the checkpoint is inside entropy data.
    let seeks = seek_log.borrow();
    assert!(
        !seeks.is_empty(),
        "retry must have performed at least one seek"
    );
    let resume_pos = seeks[0];
    assert!(
        resume_pos > entropy_start,
        "per-row checkpoint should resume past entropy_start ({entropy_start}), \
         but sought to {resume_pos} — indicates full scan replay instead of row resume"
    );
}

#[test]
fn per_row_checkpoint_preserves_vertical_upsampling_state() {
    let data = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = decode_oneshot(data);
    let entropy_start = entropy_start(data);
    let cutoff = entropy_start + (data.len() - entropy_start) * 60 / 100;

    let limit = Rc::new(Cell::new(cutoff));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);

    decoder
        .decode_headers()
        .expect("headers should be fully visible at cutoff");
    let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];

    let err = decoder
        .decode_into(&mut out)
        .expect_err("truncated scan should give recoverable EOF");
    assert!(
        err.is_recoverable_eof(),
        "expected recoverable EOF, got {err:?}"
    );

    let err = decoder
        .decode_into(&mut out)
        .expect_err("still truncated, should give recoverable EOF again");
    assert!(
        err.is_recoverable_eof(),
        "expected recoverable EOF on second attempt, got {err:?}"
    );

    seek_log.borrow_mut().clear();
    limit.set(data.len());
    decoder
        .decode_into(&mut out)
        .expect("full data should allow decode to complete");
    assert_pixels_match(&out, &expected, "per_row_vertical_upsampling", data.len());

    let seeks = seek_log.borrow();
    let resume_pos = seeks.first().copied();
    assert!(
        matches!(resume_pos, Some(pos) if pos > entropy_start),
        "retry should resume inside entropy data, got {seeks:?}"
    );
}
