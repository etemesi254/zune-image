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

use zune_core::bytestream::{ZByteReaderTrait, ZCursor};
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{JpegDecoder, NeverCancel, RawDecodeSession, RawImcuRowStatus};

use std::cell::{Cell, RefCell};
use std::io::{BufRead, Cursor, Read, Seek, SeekFrom};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

struct PositionFailCursor<'a> {
    inner:       Cursor<&'a [u8]>,
    calls:       Rc<Cell<usize>>,
    fail_on_call: Rc<Cell<usize>>
}

impl Read for PositionFailCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl BufRead for PositionFailCursor<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt);
    }
}

impl Seek for PositionFailCursor<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if position == SeekFrom::Current(0) {
            let call = self.calls.get() + 1;
            self.calls.set(call);
            if self.fail_on_call.get() == call {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "injected position failure"
                ));
            }
        }
        self.inner.seek(position)
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

fn allocate_raw_planes<T>(raw: &RawDecodeSession<'_, T>) -> Vec<Vec<u8>>
where
    T: ZByteReaderTrait
{
    let layout = raw.layout().unwrap();
    let n_components = raw.num_components().unwrap();
    (0..n_components)
        .map(|index| vec![0u8; layout[index].byte_size])
        .collect()
}

fn decode_raw_into_existing<T>(
    raw: &mut RawDecodeSession<'_, T>, planes: &mut [Vec<u8>]
) -> Result<(), zune_jpeg::errors::DecodeErrors>
where
    T: ZByteReaderTrait
{
    let mut plane_refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes(&mut plane_refs)
}

fn allocate_raw_strided_planes<T>(raw: &RawDecodeSession<'_, T>) -> (Vec<Vec<u8>>, Vec<usize>)
where
    T: ZByteReaderTrait
{
    let layout = raw.layout().unwrap();
    let n_components = raw.num_components().unwrap();
    let strides: Vec<usize> = (0..n_components)
        .map(|index| layout[index].width + 5)
        .collect();
    let planes = (0..n_components)
        .map(|index| vec![0xAAu8; strides[index] * layout[index].height])
        .collect();
    (planes, strides)
}

fn decode_raw_strided_into_existing<T>(
    raw: &mut RawDecodeSession<'_, T>, planes: &mut [Vec<u8>], strides: &[usize]
) -> Result<(), zune_jpeg::errors::DecodeErrors>
where
    T: ZByteReaderTrait
{
    let mut plane_refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes_strided(&mut plane_refs, strides)
}

fn decode_raw_oneshot(data: &[u8]) -> Vec<Vec<u8>> {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder
        .decode_headers()
        .expect("one-shot raw headers failed");
    let mut raw = decoder.raw_output();
    let mut planes = allocate_raw_planes(&raw);
    decode_raw_into_existing(&mut raw, &mut planes).expect("one-shot raw decode failed");
    planes
}

fn assert_raw_planes_match(actual: &[Vec<u8>], expected: &[Vec<u8>], name: &str, available: usize) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{name}: incremental and one-shot raw plane counts differ"
    );

    for (plane_index, (actual_plane, expected_plane)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            actual_plane.len(),
            expected_plane.len(),
            "{name}: raw plane {plane_index} lengths differ"
        );
        if let Some(index) = actual_plane
            .iter()
            .zip(expected_plane)
            .position(|(left, right)| left != right)
        {
            let start = index.saturating_sub(8);
            let end = (index + 9).min(actual_plane.len());
            panic!(
                "{name}: first raw plane {plane_index} mismatch at {index} with {available} bytes visible: incremental={}, one-shot={}, incremental window={:?}, one-shot window={:?}",
                actual_plane[index],
                expected_plane[index],
                &actual_plane[start..end],
                &expected_plane[start..end]
            );
        }
    }
}

fn assert_raw_strided_planes_match_padded(
    actual: &[Vec<u8>], strides: &[usize], expected: &[Vec<u8>], name: &str, data: &[u8],
    available: usize
) {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    let raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let n_components = raw.num_components().unwrap();

    assert_eq!(
        actual.len(),
        n_components,
        "{name}: strided plane count differs"
    );
    assert_eq!(strides.len(), n_components, "{name}: stride count differs");
    assert_eq!(
        expected.len(),
        n_components,
        "{name}: padded plane count differs"
    );

    for index in 0..n_components {
        let plane = layout[index];
        assert_eq!(
            actual[index].len(),
            strides[index] * plane.height,
            "{name}: strided plane {index} length differs"
        );
        for y in 0..plane.height {
            for x in 0..plane.width {
                let actual_value = actual[index][y * strides[index] + x];
                let expected_value = expected[index][y * plane.stride + x];
                assert_eq!(
                    actual_value, expected_value,
                    "{name}: strided raw plane {index} mismatch at ({x},{y}) with {available} bytes visible"
                );
            }
            for x in plane.width..strides[index] {
                assert_eq!(
                    actual[index][y * strides[index] + x],
                    0xAA,
                    "{name}: strided padding modified in plane {index} at ({x},{y})"
                );
            }
        }
    }
}

fn assert_incremental_raw_decode_matches_oneshot(name: &str, data: &[u8], step: usize) {
    assert!(step > 0, "incremental step must be non-zero");

    let expected = decode_raw_oneshot(data);
    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    let mut available = 0_usize;

    loop {
        available = (available + step).min(data.len());
        limit.set(available);

        match decoder.decode_headers() {
            Ok(()) => break,
            Err(ref err) if err.is_recoverable_eof() => {
                assert!(
                    available < data.len(),
                    "raw headers still need more data with the full input visible"
                );
            }
            Err(err) => panic!("unexpected raw header error at byte {available}: {err:?}")
        }
    }

    let mut raw = decoder.raw_output();
    let mut planes = allocate_raw_planes(&raw);

    loop {
        match decode_raw_into_existing(&mut raw, &mut planes) {
            Ok(()) => {
                assert_raw_planes_match(&planes, &expected, name, available);
                return;
            }
            Err(ref err) if err.is_recoverable_eof() => {
                assert!(
                    available < data.len(),
                    "raw scan still needs more data with the full input visible"
                );
                available = (available + step).min(data.len());
                limit.set(available);
            }
            Err(err) => panic!("unexpected raw scan error at byte {available}: {err:?}")
        }
    }
}

fn assert_incremental_raw_strided_decode_matches_oneshot(name: &str, data: &[u8], step: usize) {
    assert!(step > 0, "incremental step must be non-zero");

    let expected = decode_raw_oneshot(data);
    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);

    let mut available = 0_usize;

    loop {
        available = (available + step).min(data.len());
        limit.set(available);

        match decoder.decode_headers() {
            Ok(()) => break,
            Err(ref err) if err.is_recoverable_eof() => {
                assert!(
                    available < data.len(),
                    "raw strided headers still need more data with the full input visible"
                );
            }
            Err(err) => {
                panic!("unexpected raw strided header error at byte {available}: {err:?}")
            }
        }
    }

    let mut raw = decoder.raw_output();
    let (mut planes, strides) = allocate_raw_strided_planes(&raw);

    loop {
        match decode_raw_strided_into_existing(&mut raw, &mut planes, &strides) {
            Ok(()) => {
                assert_raw_strided_planes_match_padded(
                    &planes, &strides, &expected, name, data, available
                );
                return;
            }
            Err(ref err) if err.is_recoverable_eof() => {
                assert!(
                    available < data.len(),
                    "raw strided scan still needs more data with the full input visible"
                );
                available = (available + step).min(data.len());
                limit.set(available);
            }
            Err(err) => panic!("unexpected raw strided scan error at byte {available}: {err:?}")
        }
    }
}

fn assert_incremental_raw_pull_matches_oneshot(name: &str, data: &[u8], step: usize) {
    let expected = decode_raw_oneshot(data);
    let limit = Rc::new(Cell::new(0usize));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    let mut available = 0usize;

    loop {
        available = (available + step).min(data.len());
        limit.set(available);
        match decoder.decode_headers() {
            Ok(()) => break,
            Err(error) if error.is_recoverable_eof() => continue,
            Err(error) => panic!("{name}: unexpected header error: {error:?}")
        }
    }

    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    let mut actual: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.height])
        .collect();
    let mut rows_emitted = [0usize; 4];
    let mut suspended_before_first_row = false;

    loop {
        let mut stripe_storage: Vec<Vec<u8>> = layout[..count]
            .iter()
            .map(|plane| vec![0xCD; plane.width * plane.vertical_sampling_factor * 8])
            .collect();
        let mut refs: Vec<&mut [u8]> = stripe_storage.iter_mut().map(Vec::as_mut_slice).collect();

        match raw.decode_next_imcu_row(&mut refs, &strides).unwrap() {
            RawImcuRowStatus::RowReady { rows_written } => {
                for index in 0..count {
                    let bytes = rows_written[index] * strides[index];
                    let dst = rows_emitted[index] * strides[index];
                    actual[index][dst..dst + bytes]
                        .copy_from_slice(&stripe_storage[index][..bytes]);
                    rows_emitted[index] += rows_written[index];
                }
            }
            RawImcuRowStatus::NeedMoreInput => {
                assert!(
                    stripe_storage.iter().flatten().all(|byte| *byte == 0xCD),
                    "{name}: suspended pull modified caller storage"
                );
                if rows_emitted[..count].iter().all(|rows| *rows == 0) {
                    suspended_before_first_row = true;
                }
                assert!(available < data.len(), "{name}: suspended with full input");
                available = (available + step).min(data.len());
                limit.set(available);
            }
            RawImcuRowStatus::Complete => {
                assert!(
                    suspended_before_first_row,
                    "{name}: test did not suspend before publishing its first row"
                );
                assert_raw_strided_planes_match_padded(
                    &actual, &strides, &expected, name, data, available
                );
                return;
            }
            _ => unreachable!("unknown raw iMCU-row status")
        }
    }
}

fn assert_decode_raw_replay_matches_oneshot(name: &str, data: &[u8]) {
    let expected = decode_raw_oneshot(data);

    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let mut first = allocate_raw_planes(&raw);
    decode_raw_into_existing(&mut raw, &mut first).unwrap();
    assert_raw_planes_match(&first, &expected, name, data.len());

    let mut replay = allocate_raw_planes(&raw);
    decode_raw_into_existing(&mut raw, &mut replay).unwrap();
    assert_raw_planes_match(&replay, &expected, name, data.len());
}

fn assert_decode_raw_strided_replay_matches_oneshot(name: &str, data: &[u8]) {
    let expected = decode_raw_oneshot(data);

    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let (mut first, strides) = allocate_raw_strided_planes(&raw);
    decode_raw_strided_into_existing(&mut raw, &mut first, &strides).unwrap();
    assert_raw_strided_planes_match_padded(&first, &strides, &expected, name, data, data.len());

    let (mut replay, replay_strides) = allocate_raw_strided_planes(&raw);
    assert_eq!(replay_strides, strides);
    decode_raw_strided_into_existing(&mut raw, &mut replay, &replay_strides).unwrap();
    assert_raw_strided_planes_match_padded(
        &replay,
        &replay_strides,
        &expected,
        name,
        data,
        data.len()
    );
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
fn input_cut_inside_fill_bytes_resumes_correctly() {
    // The second scan of this image has four 0xFF bytes at offset 4014 followed by
    // zeros. When the visible input ends inside that run, the byte after an 0xFF is
    // not there yet; it must not be taken as a stuffed zero (0xFF 0x00 = data byte
    // 0xFF). After a retry with the whole input, the pixels must match a one-shot decode.
    let data = include_bytes!("../../../test-images/jpeg/fill_bytes_at_incremental_cut.jpg");
    let expected = decode_oneshot(data);
    for cutoff in 4014..=4018 {
        let limit = Rc::new(Cell::new(data.len()));
        let cursor = GrowableCursor::new(data, Rc::clone(&limit));
        let mut decoder = JpegDecoder::new(cursor);
        decoder.set_incremental_mode(true);
        decoder
            .decode_headers()
            .expect("headers are before the cutoff");
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        limit.set(cutoff);
        // The cut call must report a recoverable end of input: success or another error
        // would be hidden by the full-input call below, which replays the decode.
        match decoder.decode_into(&mut out) {
            Err(e) => assert!(
                e.is_recoverable_eof(),
                "cutoff {cutoff}: expected a recoverable EOF, got {e:?}"
            ),
            Ok(()) => panic!("cutoff {cutoff}: decode succeeded on truncated input")
        }
        limit.set(data.len());
        decoder
            .decode_into(&mut out)
            .expect("full input should complete the decode");
        assert_pixels_match(&out, &expected, "fill_bytes_at_incremental_cut", cutoff);
    }
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

#[test]
fn decode_raw_replay_after_success_matches_oneshot() {
    assert_decode_raw_replay_matches_oneshot(
        "baseline_raw_replay",
        include_bytes!("../../../test-images/jpeg/synthetic_image.jpg")
    );
    assert_decode_raw_replay_matches_oneshot(
        "progressive_raw_replay",
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg")
    );
}

#[test]
fn decode_raw_strided_replay_after_success_matches_oneshot() {
    assert_decode_raw_strided_replay_matches_oneshot(
        "baseline_raw_strided_replay",
        include_bytes!("../../../test-images/jpeg/fox410.jpg")
    );
    assert_decode_raw_strided_replay_matches_oneshot(
        "progressive_raw_strided_replay",
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

#[test]
fn progressive_resume_keeps_first_sos_quantization_tables() {
    // Insert a DQT redefining table 0 before the second scan of a progressive image.
    // Components keep the tables in effect at the first SOS, so a decode resumed at
    // a later scan must use those, not the later DQT.
    let base = include_bytes!("../../../test-images/jpeg/weird_sampling_3.jpg");
    let second_sos = base
        .windows(2)
        .enumerate()
        .filter(|(_, w)| *w == [0xFF, 0xDA])
        .nth(1)
        .expect("progressive image has a second SOS")
        .0;
    let mut dqt_body = vec![0x00];
    dqt_body.extend_from_slice(&[99; 64]);
    let mut data = base[..second_sos].to_vec();
    data.extend_from_slice(&marker_segment(0xDB, &dqt_body));
    data.extend_from_slice(&base[second_sos..]);
    let data = &data[..];

    let expected = decode_oneshot(data);
    for cutoff in (second_sos + 69..data.len()).step_by(16) {
        let limit = Rc::new(Cell::new(data.len()));
        let cursor = GrowableCursor::new(data, Rc::clone(&limit));
        let mut decoder = JpegDecoder::new(cursor);
        decoder.set_incremental_mode(true);
        decoder
            .decode_headers()
            .expect("headers are before the cutoff");
        let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
        limit.set(cutoff);
        let _ = decoder.decode_into(&mut out);
        limit.set(data.len());
        decoder
            .decode_into(&mut out)
            .expect("full input should complete the decode");
        assert_pixels_match(&out, &expected, "progressive_dqt_between_scans", cutoff);
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

fn inject_segments_at(base: &[u8], offset: usize, segments: &[Vec<u8>]) -> Vec<u8> {
    let extra_len = segments.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(base.len() + extra_len);
    out.extend_from_slice(&base[..offset]);
    for segment in segments {
        out.extend_from_slice(segment);
    }
    out.extend_from_slice(&base[offset..]);
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
    app1_extended_xmp_part(guid, payload.len() as u32, 0, payload)
}

fn app1_extended_xmp_part(
    guid: &[u8; 32], total_size: u32, offset: u32, payload: &[u8]
) -> Vec<u8> {
    let mut body = b"http://ns.adobe.com/xmp/extension/\0".to_vec();
    body.extend_from_slice(guid);
    body.extend_from_slice(&total_size.to_be_bytes());
    body.extend_from_slice(&offset.to_be_bytes());
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

fn with_dnl_and_trailing_segments(
    base: &[u8], segments: &[Vec<u8>],
) -> (Vec<u8>, usize, usize, u16, u16) {
    let sof = list_jpeg_markers(base)
        .into_iter()
        .find(|(_, code, _)| {
            (0xC0..=0xC2).contains(code) || (0xC9..=0xCA).contains(code)
        })
        .map(|(offset, _, _)| offset)
        .expect("JPEG must contain a supported SOF marker");
    let height = u16::from_be_bytes([base[sof + 5], base[sof + 6]]);
    let width = u16::from_be_bytes([base[sof + 7], base[sof + 8]]);
    let eoi = base
        .windows(2)
        .rposition(|marker| marker == [0xFF, 0xD9])
        .expect("baseline JPEG must contain EOI");

    let mut data = base.to_vec();
    data[sof + 5..sof + 7].fill(0);
    data.truncate(eoi);
    data.extend([0xFF, 0xDC, 0x00, 0x04]);
    data.extend(height.to_be_bytes());
    let trailing_start = data.len();
    for segment in segments {
        data.extend_from_slice(segment);
    }
    let eoi_offset = data.len();
    data.extend_from_slice(&base[eoi..]);
    (data, trailing_start, eoi_offset, width, height)
}

fn with_pre_sos_dnl(base: &[u8]) -> (Vec<u8>, u16) {
    let sof = list_jpeg_markers(base)
        .into_iter()
        .find(|(_, code, _)| {
            (0xC0..=0xC2).contains(code) || (0xC9..=0xCA).contains(code)
        })
        .map(|(offset, _, _)| offset)
        .expect("JPEG must contain a supported SOF marker");
    let height = u16::from_be_bytes([base[sof + 5], base[sof + 6]]);
    let sos = base
        .windows(2)
        .position(|marker| marker == [0xFF, 0xDA])
        .expect("JPEG must contain SOS");
    let mut data = base.to_vec();
    data[sof + 5..sof + 7].fill(0);
    data.splice(
        sos..sos,
        [0xFF, 0xDC, 0x00, 0x04, (height >> 8) as u8, height as u8],
    );
    (data, height)
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

#[test]
fn extended_xmp_parts_reassemble_after_incremental_header_retries() {
    let base = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let guid = *b"0123456789abcdef0123456789abcdef";
    let payload = b"out-of-order extended XMP survives byte-wise header retries";
    let split_a = 17;
    let split_b = 41;
    let data = inject_header_segments(
        base,
        &[
            app1_extended_xmp_part(
                &guid,
                payload.len() as u32,
                split_b as u32,
                &payload[split_b..]
            ),
            app1_extended_xmp_part(&guid, payload.len() as u32, 0, &payload[..split_a]),
            app1_extended_xmp_part(
                &guid,
                payload.len() as u32,
                split_a as u32,
                &payload[split_a..split_b]
            ),
        ]
    );

    let limit = Rc::new(Cell::new(0));
    let cursor = GrowableCursor::new(&data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    for available in 1..=data.len() {
        limit.set(available);
        match decoder.decode_headers() {
            Ok(()) => break,
            Err(error) if error.is_recoverable_eof() => {}
            Err(error) => panic!("unexpected error at byte {available}: {error:?}")
        }
    }

    let info = decoder.info().unwrap();
    assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
    assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));
}

fn check_extended_xmp_after_scan_marker(base: &[u8], insertion_offset: usize) {
    let guid = *b"0123456789abcdef0123456789abcdef";
    let payload = b"extended XMP completed after entropy decoding started";
    let split_a = 13;
    let split_b = 37;
    let segments = [
        app1_extended_xmp_part(
            &guid,
            payload.len() as u32,
            split_b as u32,
            &payload[split_b..]
        ),
        app1_extended_xmp_part(&guid, payload.len() as u32, 0, &payload[..split_a]),
        app1_extended_xmp_part(
            &guid,
            payload.len() as u32,
            split_a as u32,
            &payload[split_a..split_b]
        ),
    ];
    let data = inject_segments_at(base, insertion_offset, &segments);

    let mut one_shot = JpegDecoder::new(ZCursor::new(&data));
    one_shot.decode().unwrap();
    let info = one_shot.info().unwrap();
    assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
    assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));

    let cutoff = insertion_offset + segments[0].len() + segments[1].len() / 2;
    let limit = Rc::new(Cell::new(cutoff));
    let cursor = GrowableCursor::new(&data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);
    decoder.decode_headers().unwrap();
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(error.is_recoverable_eof(), "{error:?}");
    assert!(decoder.info().unwrap().extended_xmp.is_none());

    limit.set(data.len());
    decoder.decode_into(&mut output).unwrap();
    let info = decoder.info().unwrap();
    assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
    assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));
}

#[test]
fn extended_xmp_between_progressive_scans_reassembles_after_retry() {
    let base = include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg");
    let second_sos = base
        .windows(2)
        .enumerate()
        .filter_map(|(offset, marker)| (marker == [0xFF, 0xDA]).then_some(offset))
        .nth(1)
        .unwrap();
    check_extended_xmp_after_scan_marker(base, second_sos);
}

#[test]
fn extended_xmp_before_eoi_reassembles_after_retry() {
    let base = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    check_extended_xmp_after_scan_marker(base, eoi);
}

#[test]
fn extended_xmp_after_interleaved_scan_reassembles_after_retry() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    assert_eq!(
        base.windows(2).filter(|marker| *marker == [0xFF, 0xDA]).count(),
        1
    );
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    let mut padded = base[..eoi].to_vec();
    // Keep the entropy reader from latching the APP1 marker while decoding the
    // final MCU. The post-scan marker fallback must find and dispatch it.
    padded.extend_from_slice(&[0; 64]);
    let insertion_offset = padded.len();
    padded.extend_from_slice(&base[eoi..]);
    check_extended_xmp_after_scan_marker(&padded, insertion_offset);
}

#[test]
fn latched_extended_xmp_after_interleaved_scan_reassembles_after_retry() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    check_extended_xmp_after_scan_marker(base, eoi);
}

#[test]
fn extended_xmp_after_unhandled_app_marker_reassembles_after_retry() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    let mut padded = base[..eoi].to_vec();
    padded.extend_from_slice(&[0; 64]);
    padded.extend_from_slice(&marker_segment(0xE3, b"unhandled APP3 payload"));
    let insertion_offset = padded.len();
    padded.extend_from_slice(&base[eoi..]);
    check_extended_xmp_after_scan_marker(&padded, insertion_offset);
}

#[test]
fn unhandled_app_marker_is_accepted_by_strict_inter_scan_parser() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = decode_with_mode(base, false, true).unwrap();
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    let data = inject_segments_at(
        base,
        eoi,
        &[
            com_segment(b"enter the inter-scan marker loop"),
            marker_segment(0xE3, b"valid APP3 payload"),
        ]
    );

    let actual = decode_with_mode(&data, false, true).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn complete_extended_xmp_before_missing_eoi_preserves_lenient_completion() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let guid = *b"0123456789abcdef0123456789abcdef";
    let payload = b"complete Extended XMP followed by EOF";
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    for padding in [0, 64] {
        let mut data = base[..eoi].to_vec();
        data.resize(data.len() + padding, 0);
        data.extend_from_slice(&app1_extended_xmp(&guid, payload));

        let mut decoder = JpegDecoder::new(ZCursor::new(&data));
        decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
        assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));
    }
}

#[test]
fn complete_extended_xmp_before_eoi_rolls_back_on_incremental_eof() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let guid = *b"0123456789abcdef0123456789abcdef";
    let payload = b"complete Extended XMP retried before EOI";
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    let mut data = base[..eoi].to_vec();
    data.extend_from_slice(&[0; 64]);
    data.extend_from_slice(&app1_extended_xmp(&guid, payload));
    let cutoff = data.len();
    data.extend_from_slice(&base[eoi..]);

    let limit = Rc::new(Cell::new(cutoff));
    let cursor = GrowableCursor::new(&data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);
    decoder.decode_headers().unwrap();
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(error.is_recoverable_eof(), "{error:?}");
    assert!(decoder.info().unwrap().extended_xmp.is_none());

    limit.set(data.len());
    decoder.decode_into(&mut output).unwrap();
    let info = decoder.info().unwrap();
    assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
    assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));
}

#[test]
fn dnl_state_rolls_back_when_trailing_metadata_suspends() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let guid = *b"0123456789abcdef0123456789abcdef";
    let payload = b"Extended XMP after DNL";
    let extended_xmp = app1_extended_xmp(&guid, payload);
    let (data, app_start, eoi_offset, width, height) =
        with_dnl_and_trailing_segments(base, core::slice::from_ref(&extended_xmp));

    let reference_options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut reference = JpegDecoder::new_with_options(ZCursor::new(base), reference_options);
    let expected = reference.decode().unwrap();
    assert!(expected.iter().any(|sample| *sample != 0));

    let cutoffs = [
        ("inside trailing APP1", app_start + extended_xmp.len() / 2),
        ("before EOI", eoi_offset),
    ];
    for (mode, incremental, strict) in
        [("incremental", true, false), ("strict", false, true)]
    {
        for (boundary, cutoff) in cutoffs {
            let options = DecoderOptions::default()
                .set_max_height(usize::from(height))
                .jpeg_set_out_colorspace(ColorSpace::Luma)
                .set_strict_mode(strict);
            let limit = Rc::new(Cell::new(cutoff));
            let cursor = GrowableCursor::new(&data, Rc::clone(&limit));
            let mut decoder = JpegDecoder::new_with_options(cursor, options);
            decoder.set_incremental_mode(incremental);
            decoder.decode_headers().unwrap();
            let expected_size = usize::from(width) * usize::from(height);
            assert_eq!(decoder.output_buffer_size(), Some(expected_size));
            let mut output = vec![0xA5; decoder.output_buffer_size().unwrap()];

            let error = decoder.decode_into(&mut output).unwrap_err();
            assert!(
                error.is_recoverable_eof(),
                "{mode} at {boundary}: unexpected error {error:?}"
            );
            assert_eq!(decoder.info().unwrap().height, height);
            assert!(decoder.info().unwrap().extended_xmp.is_none());

            limit.set(data.len());
            decoder.decode_into(&mut output).unwrap_or_else(|error| {
                panic!("{mode} at {boundary}: retry failed: {error:?}")
            });
            let info = decoder.info().unwrap();
            assert_eq!(info.height, height);
            assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
            assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));
            assert_pixels_match(&output, &expected, mode, data.len());

            let mut too_small = vec![0xA5; output.len() - 1];
            let error = decoder.decode_into(&mut too_small).unwrap_err();
            assert!(
                matches!(
                    error,
                    DecodeErrors::TooSmallOutput(required, actual)
                        if required == output.len() && actual == too_small.len()
                ),
                "{mode} at {boundary}: unexpected short-buffer error {error:?}"
            );
            assert_eq!(decoder.info().unwrap().height, height);
            assert_eq!(decoder.output_buffer_size(), Some(output.len()));

            let mut replay = vec![0xA5; output.len()];
            decoder.decode_into(&mut replay).unwrap_or_else(|error| {
                panic!("{mode} at {boundary}: replay after success failed: {error:?}")
            });
            assert_pixels_match(&replay, &expected, mode, data.len());

            decoder.set_options(
                DecoderOptions::default()
                    .set_max_height(usize::from(height))
                    .jpeg_set_out_colorspace(ColorSpace::Luma)
                    .set_strict_mode(strict),
            );
            let mut option_replay = vec![0xA5; output.len()];
            decoder.decode_into(&mut option_replay).unwrap_or_else(|error| {
                panic!("{mode} at {boundary}: replay after set_options failed: {error:?}")
            });
            assert_pixels_match(&option_replay, &expected, mode, data.len());
        }
    }
}

#[test]
fn dnl_rejects_invalid_height_and_length() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let (data, dnl_end, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    let dnl = dnl_end - 6;

    let mut malformed_length = data.clone();
    malformed_length[dnl + 2..dnl + 4].copy_from_slice(&5_u16.to_be_bytes());
    let error = JpegDecoder::new(ZCursor::new(&malformed_length)).decode().unwrap_err();
    assert!(
        matches!(error, DecodeErrors::FormatStatic(message) if message.contains("DNL")),
        "invalid DNL length returned {error:?}"
    );

    let mut zero_height = data.clone();
    zero_height[dnl + 4..dnl + 6].fill(0);
    let error = JpegDecoder::new(ZCursor::new(&zero_height)).decode().unwrap_err();
    assert!(
        matches!(error, DecodeErrors::FormatStatic(message) if message.contains("nonzero")),
        "zero DNL height returned {error:?}"
    );

    let options = DecoderOptions::default().set_max_height(usize::from(height) - 1);
    let mut over_limit = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let error = over_limit.decode().unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::Format(message) if message.contains("height limit")),
        "over-limit DNL height returned {error:?}"
    );

    let mut inconsistent_rows = data;
    inconsistent_rows[dnl + 4..dnl + 6].copy_from_slice(&(height / 2).to_be_bytes());
    let options = DecoderOptions::default().set_max_height(usize::from(height));
    let mut inconsistent =
        JpegDecoder::new_with_options(ZCursor::new(&inconsistent_rows), options);
    let error = inconsistent.decode().unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::Format(message) if message.contains("MCU rows")),
        "inconsistent DNL height returned {error:?}"
    );
}

#[test]
fn unsupported_multi_scan_and_progressive_dnl_are_rejected() {
    for (name, base, expected_message) in [
        (
            "baseline multi-scan",
            include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg").as_slice(),
            "baseline multi-scan",
        ),
        (
            "progressive",
            include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg").as_slice(),
            "progressive JPEG",
        ),
    ] {
        let (data, _, _, _, _) = with_dnl_and_trailing_segments(base, &[]);
        let error = JpegDecoder::new(ZCursor::new(&data)).decode().unwrap_err();
        assert!(
            matches!(&error, DecodeErrors::FormatStatic(message) if message.contains(expected_message)),
            "{name} DNL returned {error:?}"
        );
    }
}

#[test]
fn dnl_rejects_additional_sos_on_every_marker_path() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let extra_sos =
        vec![0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00];
    let (mut before_dnl, dnl_end, _, _, height) =
        with_dnl_and_trailing_segments(base, &[]);
    before_dnl.splice(dnl_end - 6..dnl_end - 6, extra_sos.iter().copied());
    let (after_dnl, _, _, _, _) =
        with_dnl_and_trailing_segments(base, core::slice::from_ref(&extra_sos));
    let (after_setup_marker, _, _, _, _) = with_dnl_and_trailing_segments(
        base,
        &[marker_segment(0xE3, b"setup before forbidden SOS"), extra_sos],
    );

    for (path, data) in [
        ("before DNL", before_dnl),
        ("directly after DNL", after_dnl),
        ("after trailing setup marker", after_setup_marker),
    ] {
        for strict in [false, true] {
            let options = DecoderOptions::default()
                .set_max_height(usize::from(height))
                .set_strict_mode(strict);
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
            let error = decoder.decode().unwrap_err();
            assert!(
                matches!(&error, DecodeErrors::FormatStatic(message) if message.contains("multiple scans")),
                "{path}, strict={strict}: returned {error:?}"
            );
        }
    }
}

#[test]
fn strict_dnl_accepts_scan_ending_on_restart_interval() {
    let base = include_bytes!("../../../test-images/jpeg/four_components.jpg");
    let (data, _, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .set_strict_mode(true);
    let mut reference = JpegDecoder::new_with_options(ZCursor::new(base), options);
    let expected = reference.decode().unwrap();
    let mut dnl = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let actual = dnl.decode().unwrap();
    assert_pixels_match(&actual, &expected, "restart-boundary DNL", data.len());
}

#[test]
fn dnl_before_end_of_mcu_row_is_rejected() {
    let base = include_bytes!("../../../test-images/jpeg/four_components.jpg");
    let markers = list_jpeg_markers(base);
    let sof = markers
        .iter()
        .find(|(_, code, _)| *code == 0xC0)
        .map(|(offset, _, _)| *offset)
        .expect("fixture must contain SOF0");
    let rst = markers
        .iter()
        .find(|(_, code, _)| (0xD0..=0xD7).contains(code))
        .map(|(offset, _, _)| *offset)
        .expect("fixture must contain a restart marker");
    let components = usize::from(base[sof + 9]);
    let h_max = (0..components)
        .map(|index| usize::from(base[sof + 11 + index * 3] >> 4))
        .max()
        .unwrap();
    let width = u16::from_be_bytes([base[sof + 7], base[sof + 8]]);
    let widened = width + (h_max * 8) as u16;

    let mut data = base.to_vec();
    data[sof + 5..sof + 7].fill(0);
    data[sof + 7..sof + 9].copy_from_slice(&widened.to_be_bytes());
    data.splice(rst..rst + 2, [0xFF, 0xDC, 0x00, 0x04, 0x00, 0x01]);

    let options = DecoderOptions::default().set_strict_mode(true);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let error = decoder.decode().unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::FormatStatic(message) if message.contains("end of an MCU row")),
        "partial-row DNL returned {error:?}"
    );
}

#[test]
fn huffman_dnl_read_ahead_waits_for_declared_final_row() {
    let base = include_bytes!("images/dnl_read_ahead.jpg");
    let (data, _, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut reference = JpegDecoder::new_with_options(ZCursor::new(base), options);
    let expected = reference.decode().unwrap();
    let mut dnl = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let actual = dnl.decode().unwrap();
    assert_pixels_match(&actual, &expected, "Huffman DNL read-ahead", data.len());
}

#[test]
fn pending_dnl_retry_retains_pre_dnl_checkpoint() {
    let base = include_bytes!("images/dnl_read_ahead.jpg");
    let (data, dnl_end, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    let dnl_start = dnl_end - 6;
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut reference = JpegDecoder::new_with_options(ZCursor::new(base), options);
    let expected = reference.decode().unwrap();

    let limit = Rc::new(Cell::new(data.len()));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(&data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new_with_options(cursor, options);
    decoder.set_incremental_mode(true);
    decoder.decode_headers().unwrap();
    decoder.set_cancel_interval(1);
    let polls = Arc::new(AtomicUsize::new(0));
    decoder.set_cancel({
        let polls = Arc::clone(&polls);
        move || polls.fetch_add(1, Ordering::Relaxed) >= 3
    });
    let mut output = vec![0xA5; decoder.output_buffer_size().unwrap()];
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert_eq!(decoder.info().unwrap().height, height, "DNL must be resolved before cancellation");

    seek_log.borrow_mut().clear();
    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_pixels_match(&output, &expected, "pending DNL checkpoint retry", data.len());
    let retry_position = *seek_log.borrow().first().expect("retry must seek");
    assert!(
        retry_position < dnl_start,
        "retry checkpoint {retry_position} must precede consumed DNL at {dnl_start}"
    );
}

#[test]
fn pending_dnl_rejects_rows_beyond_buffered_entropy() {
    let base = include_bytes!("images/dnl_read_ahead.jpg");
    let (mut data, dnl_end, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    let declared_height = height + 8;
    data[dnl_end - 2..dnl_end].copy_from_slice(&declared_height.to_be_bytes());
    for trailing_bytes in [0, 32] {
        let mut candidate = data.clone();
        let eoi = candidate
            .windows(2)
            .rposition(|marker| marker == [0xFF, 0xD9])
            .unwrap();
        candidate.splice(eoi..eoi, core::iter::repeat(0).take(trailing_bytes));
        let options = DecoderOptions::default()
            .set_max_height(usize::from(declared_height))
            .jpeg_set_out_colorspace(ColorSpace::Luma)
            .set_strict_mode(true);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&candidate), options);
        let error = decoder.decode().unwrap_err();
        assert!(
            matches!(&error, DecodeErrors::FormatStatic(message) if message.contains("Entropy data ended")),
            "overstated pending DNL height with {trailing_bytes} trailing bytes returned {error:?}"
        );
    }
}

#[test]
#[cfg(feature = "arith")]
fn arithmetic_dnl_before_end_of_mcu_row_is_rejected() {
    let base = include_bytes!("../../../test-images/jpeg/arith/seq.jpg");
    let sof = list_jpeg_markers(base)
        .into_iter()
        .find(|(_, code, _)| *code == 0xC9)
        .map(|(offset, _, _)| offset)
        .expect("fixture must contain sequential arithmetic SOF");
    let entropy = sos_data_start(base, 0);
    let mut data = base[..entropy + 8].to_vec();
    data[sof + 5..sof + 7].fill(0);
    data.extend([0xFF, 0xDC, 0x00, 0x04, 0x00, 0x01, 0xFF, 0xD9]);

    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    let error = decoder.decode().unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::FormatStatic(message) if message.contains("end of an MCU row")),
        "partial-row arithmetic DNL returned {error:?}"
    );
}

#[test]
fn duplicate_pre_sos_dnl_must_match_resolved_height() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let (mut data, height) = with_pre_sos_dnl(base);
    let sos = list_jpeg_markers(&data)
        .into_iter()
        .find(|(_, code, _)| *code == 0xDA)
        .map(|(offset, _, _)| offset)
        .expect("fixture must contain SOS");
    let conflicting = height - 1;
    data.splice(
        sos..sos,
        [
            0xFF,
            0xDC,
            0x00,
            0x04,
            (conflicting >> 8) as u8,
            conflicting as u8,
        ],
    );

    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    let error = decoder.decode().unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::Format(message) if message.contains("changed")),
        "conflicting pre-SOS DNL returned {error:?}"
    );
}

#[test]
fn duplicate_trailing_dnl_must_match_resolved_height() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let (_, _, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    let matching = vec![
        0xFF,
        0xDC,
        0x00,
        0x04,
        (height >> 8) as u8,
        height as u8,
    ];
    let conflicting_height = height - 1;
    let conflicting = vec![
        0xFF,
        0xDC,
        0x00,
        0x04,
        (conflicting_height >> 8) as u8,
        conflicting_height as u8,
    ];

    let (matching_data, _, _, _, _) =
        with_dnl_and_trailing_segments(base, &[matching]);
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .set_strict_mode(true);
    JpegDecoder::new_with_options(ZCursor::new(&matching_data), options)
        .decode()
        .unwrap();

    let (conflicting_data, _, _, _, _) =
        with_dnl_and_trailing_segments(base, &[conflicting]);
    for strict in [false, true] {
        let options = DecoderOptions::default()
            .set_max_height(usize::from(height))
            .set_strict_mode(strict);
        let mut decoder =
            JpegDecoder::new_with_options(ZCursor::new(&conflicting_data), options);
        let error = decoder.decode().unwrap_err();
        assert!(
            matches!(&error, DecodeErrors::Format(message) if message.contains("changed")),
            "conflicting trailing DNL, strict={strict}, returned {error:?}"
        );
    }
}

#[test]
fn resolved_entropy_dnl_uses_declared_row_barrier() {
    let base = include_bytes!("images/dnl_read_ahead.jpg");
    let (mut data, height) = with_pre_sos_dnl(base);
    let eoi = data.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    data.splice(
        eoi..eoi,
        [
            0xFF,
            0xDC,
            0x00,
            0x04,
            (height >> 8) as u8,
            height as u8,
        ],
    );
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma)
        .set_strict_mode(true);
    let mut reference = JpegDecoder::new_with_options(ZCursor::new(base), options);
    let expected = reference.decode().unwrap();
    let mut duplicate = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let actual = duplicate.decode().unwrap();
    assert_pixels_match(&actual, &expected, "resolved entropy DNL", data.len());

    let entropy = sos_data_start(&data, 0);
    let mut premature = data[..entropy + 1].to_vec();
    premature.extend([
        0xFF,
        0xDC,
        0x00,
        0x04,
        (height >> 8) as u8,
        height as u8,
        0xFF,
        0xD9,
    ]);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&premature), options);
    let error = decoder.decode().unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::FormatStatic(message) if message.contains("Entropy data ended")),
        "premature resolved entropy DNL returned {error:?}"
    );
}

#[test]
fn pre_sos_dnl_keeps_persistent_identity() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let (data, height) = with_pre_sos_dnl(base);
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut reference = JpegDecoder::new_with_options(ZCursor::new(base), options);
    let expected = reference.decode().unwrap();
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let actual = decoder.decode().unwrap();
    assert_pixels_match(&actual, &expected, "pre-SOS DNL", data.len());

    let mut raw = decoder.raw_output();
    let mut planes = [];
    assert!(matches!(
        raw.decode_into_planes(&mut planes),
        Err(DecodeErrors::FormatStatic(
            "raw output does not support DNL images"
        ))
    ));
    drop(raw);

    decoder.set_options(
        DecoderOptions::default()
            .set_max_height(usize::from(height) - 1)
            .jpeg_set_out_colorspace(ColorSpace::Luma),
    );
    let mut replay = vec![0xA5; actual.len()];
    let error = decoder.decode_into(&mut replay).unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::Format(message) if message.contains("height limit")),
        "lowered pre-SOS DNL limit returned {error:?}"
    );
}

#[test]
fn dnl_vertical_subsampling_final_rows_match_original() {
    let base = include_bytes!(
        "../../../test-images/jpeg/large_vertical_samp_7680_4320.jpg"
    );
    let (data, _, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    assert_eq!(height % 16, 0, "fixture must end on a vertically sampled MCU row");

    let expected = JpegDecoder::new(ZCursor::new(base)).decode().unwrap();
    let actual = JpegDecoder::new(ZCursor::new(&data)).decode().unwrap();
    assert_pixels_match(&actual, &expected, "vertical-subsampling DNL", data.len());
}

#[test]
fn sampled_grayscale_dnl_matches_original() {
    let base = include_bytes!(
        "../../../test-images/jpeg/app14/single_component_transform_zero.jpg"
    );
    let (data, _, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut reference = JpegDecoder::new_with_options(ZCursor::new(base), options);
    let expected = reference.decode().unwrap();
    let mut dnl = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let actual = dnl.decode().unwrap();
    assert_pixels_match(&actual, &expected, "sampled grayscale DNL", data.len());
}

#[test]
fn dnl_replay_rejects_lowered_height_limit() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let (data, _, _, width, height) = with_dnl_and_trailing_segments(base, &[]);
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let expected = decoder.decode().unwrap();

    decoder.set_options(
        DecoderOptions::default()
            .set_max_height(usize::from(height) - 1)
            .jpeg_set_out_colorspace(ColorSpace::Luma),
    );
    assert_eq!(decoder.output_buffer_size(), Some(usize::from(width) * usize::from(height)));
    let mut output = vec![0xA5; expected.len()];
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::Format(message) if message.contains("height limit")),
        "lowered DNL limit returned {error:?}"
    );
    assert_eq!(decoder.info().unwrap().height, height);

    decoder.set_options(options);
    decoder.decode_into(&mut output).unwrap();
    assert_pixels_match(&output, &expected, "DNL lowered-limit retry", data.len());
}

#[test]
fn dnl_cancelled_replay_preserves_resolved_geometry() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let (data, _, _, width, height) = with_dnl_and_trailing_segments(base, &[]);
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    let expected = decoder.decode().unwrap();

    decoder.set_cancel_interval(1);
    decoder.set_cancel(|| true);
    let mut output = vec![0xA5; expected.len()];
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert_eq!(decoder.info().unwrap().height, height);
    assert_eq!(decoder.dimensions(), Some((usize::from(width), usize::from(height))));
    assert_eq!(decoder.output_buffer_size(), Some(expected.len()));

    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_pixels_match(&output, &expected, "DNL cancellation retry", data.len());
}

#[test]
fn raw_output_rejects_dnl_after_pixel_decode() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let (data, _, _, _, height) = with_dnl_and_trailing_segments(base, &[]);
    let options = DecoderOptions::default()
        .set_max_height(usize::from(height))
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
    decoder.decode().unwrap();

    let mut raw = decoder.raw_output();
    let mut planes = [];
    assert!(matches!(
        raw.decode_into_planes(&mut planes),
        Err(DecodeErrors::FormatStatic(
            "raw output does not support DNL images"
        ))
    ));
    assert!(matches!(
        raw.decode_into_planes_strided(&mut planes, &[]),
        Err(DecodeErrors::FormatStatic(
            "raw output does not support DNL images"
        ))
    ));
    assert!(matches!(
        raw.decode_next_imcu_row(&mut planes, &[]),
        Err(DecodeErrors::FormatStatic(
            "raw output does not support DNL images"
        ))
    ));
}

#[test]
fn too_small_replay_preserves_trailing_metadata() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let icc_payload = b"trailing ICC survives output preflight";
    let gain_payload = b"\x00\x00\x00\x01trailing gain map survives output preflight";
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    let data = inject_segments_at(
        base,
        eoi,
        &[icc_app2_chunk(icc_payload), app2_gain_map(gain_payload)],
    );
    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    let expected = decoder.decode().unwrap();
    assert_eq!(decoder.icc_profile().as_deref(), Some(icc_payload.as_slice()));
    assert_eq!(decoder.info().unwrap().gain_map_info.len(), 1);
    assert_eq!(
        decoder.info().unwrap().gain_map_info[0].data.as_slice(),
        gain_payload
    );

    let mut too_small = vec![0xA5; expected.len() - 1];
    let error = decoder.decode_into(&mut too_small).unwrap_err();
    assert!(matches!(error, DecodeErrors::TooSmallOutput(_, _)));
    assert_eq!(decoder.icc_profile().as_deref(), Some(icc_payload.as_slice()));
    assert_eq!(decoder.info().unwrap().gain_map_info.len(), 1);
    assert_eq!(
        decoder.info().unwrap().gain_map_info[0].data.as_slice(),
        gain_payload
    );

    let mut replay = vec![0xA5; expected.len()];
    decoder.decode_into(&mut replay).unwrap();
    assert_pixels_match(&replay, &expected, "trailing metadata replay", data.len());
}

fn check_complete_extended_xmp_before_missing_eoi(base: &[u8]) {
    let guid = *b"0123456789abcdef0123456789abcdef";
    let payload = b"complete multipart Extended XMP before a missing EOI";
    let split = 23;
    let parts = [
        app1_extended_xmp_part(
            &guid,
            payload.len() as u32,
            split as u32,
            &payload[split..]
        ),
        app1_extended_xmp_part(&guid, payload.len() as u32, 0, &payload[..split]),
    ];
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    let without_eoi = inject_segments_at(&base[..eoi], eoi, &parts);

    let mut lenient = JpegDecoder::new(ZCursor::new(&without_eoi));
    lenient.decode().unwrap();
    let info = lenient.info().unwrap();
    assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
    assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));

    let strict_options = DecoderOptions::default().set_strict_mode(true);
    let mut strict =
        JpegDecoder::new_with_options(ZCursor::new(&without_eoi), strict_options);
    assert!(strict.decode().unwrap_err().is_recoverable_eof());
    assert!(strict.info().unwrap().extended_xmp.is_none());

    let mut complete = without_eoi.clone();
    complete.extend_from_slice(&base[eoi..]);
    let limit = Rc::new(Cell::new(without_eoi.len()));
    let cursor = GrowableCursor::new(&complete, Rc::clone(&limit));
    let mut incremental = JpegDecoder::new(cursor);
    incremental.set_incremental_mode(true);
    incremental.decode_headers().unwrap();
    let mut output = vec![0; incremental.output_buffer_size().unwrap()];
    let error = incremental.decode_into(&mut output).unwrap_err();
    assert!(error.is_recoverable_eof(), "{error:?}");
    assert!(incremental.info().unwrap().extended_xmp.is_none());

    limit.set(complete.len());
    incremental.decode_into(&mut output).unwrap();
    let info = incremental.info().unwrap();
    assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
    assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));
}

#[test]
fn complete_extended_xmp_before_missing_eoi_in_baseline_multi_sos() {
    check_complete_extended_xmp_before_missing_eoi(include_bytes!(
        "../../../test-images/jpeg/tiny_non_interleaved_444.jpg"
    ));
}

#[test]
fn complete_extended_xmp_before_missing_eoi_in_progressive_huffman() {
    check_complete_extended_xmp_before_missing_eoi(include_bytes!(
        "../../../test-images/jpeg/progressive_restart_420.jpg"
    ));
}

#[test]
#[cfg(feature = "arith")]
fn complete_extended_xmp_before_missing_eoi_in_progressive_arithmetic() {
    check_complete_extended_xmp_before_missing_eoi(include_bytes!(
        "../../../test-images/jpeg/arith/prog.jpg"
    ));
}

#[test]
fn trailing_extended_xmp_is_not_limited_to_64_markers() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let guid = *b"0123456789abcdef0123456789abcdef";
    let payload: Vec<u8> = (0..66).collect();
    let parts: Vec<Vec<u8>> = payload
        .iter()
        .enumerate()
        .map(|(offset, byte)| {
            app1_extended_xmp_part(
                &guid,
                payload.len() as u32,
                offset as u32,
                core::slice::from_ref(byte)
            )
        })
        .collect();
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    let data = inject_segments_at(base, eoi, &parts);

    for (incremental, strict) in [(false, false), (false, true), (true, false)] {
        let options = DecoderOptions::default().set_strict_mode(strict);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
        decoder.set_incremental_mode(incremental);
        decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
        assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));
    }
}

#[test]
fn multi_sos_fallback_preserves_final_marker_semantics() {
    let base = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();

    let mut many_markers = base[..eoi].to_vec();
    many_markers.extend_from_slice(&[0; 64]);
    for _ in 0..66 {
        many_markers.extend_from_slice(&marker_segment(0xE3, &[]));
    }
    many_markers.extend_from_slice(&base[eoi..]);
    JpegDecoder::new(ZCursor::new(&many_markers)).decode().unwrap();

    let guid = *b"0123456789abcdef0123456789abcdef";
    let payload = b"multi-SOS fallback metadata before lenient EOF";
    let mut missing_eoi = base[..eoi].to_vec();
    missing_eoi.extend_from_slice(&[0; 64]);
    missing_eoi.extend_from_slice(&app1_extended_xmp(&guid, payload));
    let mut decoder = JpegDecoder::new(ZCursor::new(&missing_eoi));
    decoder.decode().unwrap();
    let info = decoder.info().unwrap();
    assert_eq!(info.extended_xmp.as_deref(), Some(payload.as_slice()));
    assert_eq!(info.extended_xmp_guid.as_deref(), Some(guid.as_slice()));
}

#[test]
fn more_than_64_markers_before_an_additional_sos_is_rejected() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    let mut data = base[..eoi].to_vec();
    for _ in 0..66 {
        data.extend_from_slice(&marker_segment(0xE3, &[]));
    }
    data.extend([0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
    data.extend([0x12, 0x34, 0x56, 0x78, 0xFF, 0xD9]);

    let error = JpegDecoder::new(ZCursor::new(&data)).decode().unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::FormatStatic(message) if message.contains("Too many markers")),
        "unexpected error: {error:?}"
    );
}

#[test]
fn standalone_restart_after_trailing_comment_preserves_lenient_completion() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = decode_oneshot(base);
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();
    for padding in [0, 64] {
        let mut data = base[..eoi].to_vec();
        data.resize(data.len() + padding, 0);
        data.extend_from_slice(&com_segment(b"complete trailing marker"));
        data.extend_from_slice(&[0xFF, 0xD0]);
        data.extend_from_slice(&base[eoi..]);

        assert_eq!(decode_oneshot(&data), expected);
        assert!(decode_with_mode(&data, false, true).is_err());
        assert!(decode_with_mode(&data, true, false).is_err());
    }
}

#[test]
fn direct_extra_sos_after_interleaved_scan_preserves_lenient_completion() {
    let base = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = decode_oneshot(base);
    let sos = base.windows(2).position(|marker| marker == [0xFF, 0xDA]).unwrap();
    let sos_length = usize::from(u16::from_be_bytes([base[sos + 2], base[sos + 3]]));
    let eoi = base.windows(2).rposition(|marker| marker == [0xFF, 0xD9]).unwrap();

    let mut data = base[..eoi].to_vec();
    data.extend_from_slice(&[0; 64]);
    data.extend_from_slice(&base[sos..sos + 2 + sos_length]);
    data.extend_from_slice(&[0xFF, 0xD0]);
    data.extend_from_slice(&base[eoi..]);

    let actual = decode_oneshot(&data);
    assert_eq!(actual, expected);
    assert!(decode_with_mode(&data, false, true).is_err());
    assert!(decode_with_mode(&data, true, false).is_err());
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
fn raw_incremental_parity() {
    assert_incremental_raw_decode_matches_oneshot(
        "synthetic_image_raw",
        include_bytes!("../../../test-images/jpeg/synthetic_image.jpg"),
        37
    );
    assert_incremental_raw_decode_matches_oneshot(
        "down_sampled_grayscale_prog_raw",
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
        31
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
fn raw_strided_incremental_parity() {
    assert_incremental_raw_strided_decode_matches_oneshot(
        "fox410_raw_strided",
        include_bytes!("../../../test-images/jpeg/fox410.jpg"),
        4096
    );
    assert_incremental_raw_strided_decode_matches_oneshot(
        "down_sampled_grayscale_prog_raw_strided",
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
        31
    );
}

#[test]
fn raw_pull_incremental_parity() {
    assert_incremental_raw_pull_matches_oneshot(
        "baseline_pull",
        include_bytes!("../../../test-images/jpeg/synthetic_image.jpg"),
        37
    );
    assert_incremental_raw_pull_matches_oneshot(
        "progressive_pull",
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
        31
    );
    assert_incremental_raw_pull_matches_oneshot(
        "restart_pull",
        include_bytes!("../../../test-images/jpeg/four_components.jpg"),
        97
    );
    assert_incremental_raw_pull_matches_oneshot(
        "multi_sos_pull",
        include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
        31
    );
}

#[test]
fn raw_pull_checkpoint_failure_is_atomic() {
    let data = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let calls = Rc::new(Cell::new(0usize));
    let fail_on_call = Rc::new(Cell::new(0usize));
    let cursor = PositionFailCursor {
        inner: Cursor::new(data.as_slice()),
        calls: Rc::clone(&calls),
        fail_on_call: Rc::clone(&fail_on_call)
    };
    let mut decoder = JpegDecoder::new(cursor);
    decoder.decode_headers().unwrap();
    calls.set(0);
    fail_on_call.set(2);

    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    let mut storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0xCD; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();
    assert_eq!(
        raw.decode_next_imcu_row(&mut refs, &strides).unwrap(),
        RawImcuRowStatus::NeedMoreInput
    );
    assert!(storage.iter().flatten().all(|byte| *byte == 0xCD));

    fail_on_call.set(0);
    let mut retry_storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut retry_refs: Vec<&mut [u8]> =
        retry_storage.iter_mut().map(Vec::as_mut_slice).collect();
    assert!(matches!(
        raw.decode_next_imcu_row(&mut retry_refs, &strides).unwrap(),
        RawImcuRowStatus::RowReady { .. }
    ));
}

#[test]
#[cfg(feature = "arith")]
fn arithmetic_raw_pull_incremental_parity() {
    assert_incremental_raw_pull_matches_oneshot(
        "arithmetic_restart_pull",
        include_bytes!("../../../test-images/jpeg/arith/seq-restart.jpg"),
        23
    );
    assert_incremental_raw_pull_matches_oneshot(
        "arithmetic_progressive_pull",
        include_bytes!("../../../test-images/jpeg/arith/prog.jpg"),
        23
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

#[test]
#[cfg(feature = "arith")]
fn arithmetic_restart_raw_resume_uses_rst_checkpoint() {
    let data = include_bytes!("../../../test-images/jpeg/arith/seq-restart.jpg");
    let expected = decode_raw_oneshot(data);
    let limit = Rc::new(Cell::new(874_usize));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(data, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);

    decoder
        .decode_headers()
        .expect("headers should be visible before first raw RST retry");
    let mut raw = decoder.raw_output();
    let mut planes = allocate_raw_planes(&raw);
    let err = decode_raw_into_existing(&mut raw, &mut planes)
        .expect_err("truncated raw scan after first RST should be recoverable");
    assert!(
        err.is_recoverable_eof(),
        "expected recoverable EOF, got {err:?}"
    );

    seek_log.borrow_mut().clear();
    limit.set(data.len());
    decode_raw_into_existing(&mut raw, &mut planes)
        .expect("full input should resume raw decode from RST checkpoint");

    let seeks = seek_log.borrow();
    assert!(
        seeks.contains(&857),
        "raw retry should seek to first RST checkpoint at 857, got {seeks:?}"
    );
    assert_raw_planes_match(&planes, &expected, "arith_seq_restart_raw_checkpoint", data.len());
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
fn baseline_scan_limit_is_preserved_across_incremental_retry() {
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let options = DecoderOptions::default().jpeg_set_max_scans(2);

    let mut one_shot = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let error = one_shot.decode().unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::Format(message) if message.contains("Too many scans")),
        "one-shot decode returned {error:?}"
    );

    let cutoff = sos_data_start(data, 1) + 1;
    let limit = Rc::new(Cell::new(cutoff));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut incremental = JpegDecoder::new_with_options(cursor, options);
    incremental.set_incremental_mode(true);
    incremental.decode_headers().unwrap();
    let mut output = vec![0; incremental.output_buffer_size().unwrap()];

    let error = incremental.decode_into(&mut output).unwrap_err();
    assert!(error.is_recoverable_eof(), "first attempt returned {error:?}");

    limit.set(data.len());
    let error = incremental.decode_into(&mut output).unwrap_err();
    assert!(
        matches!(&error, DecodeErrors::Format(message) if message.contains("Too many scans")),
        "incremental retry returned {error:?}"
    );
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

fn checkpointed_pixel_decoder<'a>(
    data: &'a [u8], cutoff: usize, options: DecoderOptions
) -> (JpegDecoder<GrowableCursor<'a>>, Rc<Cell<usize>>) {
    let limit = Rc::new(Cell::new(cutoff));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new_with_options(cursor, options);
    decoder.set_incremental_mode(true);
    decoder.decode_headers().unwrap();
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    for _ in 0..2 {
        let error = decoder.decode_into(&mut output).unwrap_err();
        assert!(error.is_recoverable_eof());
    }
    assert!(decoder.decoded_scanlines().unwrap_or(0) > 0);
    (decoder, limit)
}

#[test]
fn switching_checkpointed_pixel_decode_to_whole_raw_replays_from_start() {
    let data = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let entropy_start = entropy_start(data);
    let cutoff = entropy_start + (data.len() - entropy_start) * 60 / 100;
    let expected = decode_raw_oneshot(data);
    let (mut decoder, limit) =
        checkpointed_pixel_decoder(data, cutoff, DecoderOptions::default());
    limit.set(data.len());

    let mut raw = decoder.raw_output();
    let mut actual = allocate_raw_planes(&raw);
    decode_raw_into_existing(&mut raw, &mut actual).unwrap();
    assert_raw_planes_match(&actual, &expected, "pixel_to_whole_raw", data.len());
}

#[test]
fn switching_checkpointed_pixel_decode_to_raw_pull_replays_from_start() {
    let data = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let entropy_start = entropy_start(data);
    let cutoff = entropy_start + (data.len() - entropy_start) * 60 / 100;
    let expected = decode_raw_oneshot(data);
    let (mut decoder, limit) =
        checkpointed_pixel_decoder(data, cutoff, DecoderOptions::default());
    limit.set(data.len());

    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<_> = layout[..count].iter().map(|plane| plane.width).collect();
    let mut storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut planes: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();
    let rows_written = match raw
        .decode_next_imcu_row(&mut planes, &strides)
        .unwrap()
    {
        RawImcuRowStatus::RowReady { rows_written } => rows_written,
        _ => panic!("raw pull did not replay its first stripe")
    };
    for index in 0..count {
        for row in 0..rows_written[index] {
            assert_eq!(
                &storage[index][row * strides[index]..(row + 1) * strides[index]],
                &expected[index]
                    [row * layout[index].stride..row * layout[index].stride + strides[index]]
            );
        }
    }
}

#[test]
fn changing_options_replays_pixel_output_and_invalidates_progress() {
    let baseline = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let luma = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
    let expected = decode_oneshot(baseline);
    let entropy_start = entropy_start(baseline);
    let cutoff = entropy_start + (baseline.len() - entropy_start) * 60 / 100;
    let (mut decoder, limit) = checkpointed_pixel_decoder(baseline, cutoff, luma);
    limit.set(baseline.len());
    decoder.set_options(DecoderOptions::default());
    assert_eq!(decoder.decoded_output_bytes(), Some(0));
    assert_eq!(decoder.decoded_scanlines(), Some(0));
    let mut output = vec![0; expected.len()];
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);

    let progressive =
        include_bytes!("../../../test-images/jpeg/rebuilt_relax_fill_bytes_before_marker.jpg");
    let scans = progressive_sos_scans(progressive);
    let cutoff = scans[1].data_start + 1;
    let limit = Rc::new(Cell::new(cutoff));
    let cursor = GrowableCursor::new(progressive, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new_with_options(cursor, luma);
    decoder.set_incremental_mode(true);
    decoder.decode_headers().unwrap();
    let mut preview = vec![0; decoder.output_buffer_size().unwrap()];
    let error = decoder.decode_into(&mut preview).unwrap_err();
    assert!(error.is_recoverable_eof());
    let committed_scans = decoder.decoded_scans().unwrap();
    assert!(committed_scans > 0);
    assert!(decoder.decoded_preview_output_bytes().unwrap_or(0) > 0);

    let rgba = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    decoder.set_options(rgba);
    assert_eq!(decoder.decoded_output_bytes(), Some(0));
    assert_eq!(decoder.decoded_scanlines(), Some(0));
    assert_eq!(decoder.decoded_scans(), Some(committed_scans));
    assert_eq!(decoder.decoded_preview_output_bytes(), Some(0));
    assert_eq!(decoder.decoded_preview_scanlines(), Some(0));
    limit.set(progressive.len());
    let expected = JpegDecoder::new_with_options(ZCursor::new(progressive), rgba)
        .decode()
        .unwrap();
    let mut output = vec![0; expected.len()];
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
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

#[test]
fn stable_prefix_excludes_rows_decoded_past_eof() {
    // With the input cut a few bytes into the last MCU row, the row can still be
    // completed from the bit reader's zero padding. Those pixels change once the
    // real bits arrive, so they must not be reported as stable.
    for (name, data, cutoffs) in [
        (
            "sampling_factors",
            include_bytes!("../../../test-images/jpeg/sampling_factors.jpg").as_slice(),
            3000..3060
        ),
        (
            "weid_sampling_factors",
            include_bytes!("../../../test-images/jpeg/weid_sampling_factors.jpg").as_slice(),
            1700..1760
        )
    ] {
        let expected = decode_oneshot(data);
        for cutoff in cutoffs {
            let limit = Rc::new(Cell::new(cutoff));
            let cursor = GrowableCursor::new(data, Rc::clone(&limit));
            let mut decoder = JpegDecoder::new(cursor);
            decoder.set_incremental_mode(true);
            decoder
                .decode_headers()
                .expect("headers are before the cutoff");
            let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];
            if decoder.decode_into(&mut out).is_ok() {
                continue;
            }
            let stable = decoder.decoded_output_bytes().unwrap();
            if let Some(index) = out[..stable]
                .iter()
                .zip(&expected)
                .position(|(left, right)| left != right)
            {
                panic!(
                    "{name}: {stable} stable bytes with {cutoff} of {} bytes visible, but byte {index} changes later",
                    data.len()
                );
            }
        }
    }
}
