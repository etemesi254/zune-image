/*
 * Copyright (c) 2026.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::io::{BufRead, Cursor, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use zune_core::bytestream::ZCursor;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{CancelCheck, JpegDecoder, NeverCancel};

const BASELINE: &[u8] = include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg");
const PROGRESSIVE: &[u8] =
    include_bytes!("../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg");
const PROGRESSIVE_GRAYSCALE: &[u8] =
    include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg");
#[cfg(feature = "arith")]
const PROGRESSIVE_ARITHMETIC: &[u8] =
    include_bytes!("../../../test-images/jpeg/arith/prog.jpg");

struct PositionCountingReader<'a> {
    inner:           Cursor<&'a [u8]>,
    position_queries: Arc<AtomicUsize>
}

impl Read for PositionCountingReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buffer)
    }
}

impl BufRead for PositionCountingReader<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amount: usize) {
        self.inner.consume(amount);
    }
}

impl Seek for PositionCountingReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if matches!(position, SeekFrom::Current(0)) {
            self.position_queries.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.seek(position)
    }
}

/// Returns a check that cancels after `n` polls.
fn cancel_after(n: usize) -> impl Fn() -> bool + Send + Sync {
    let remaining = Arc::new(AtomicUsize::new(n));
    move || {
        if remaining.load(Ordering::Relaxed) == 0 {
            true
        } else {
            remaining.fetch_sub(1, Ordering::Relaxed);
            false
        }
    }
}

fn decode_with(data: &[u8], cancel: impl CancelCheck + 'static) -> Result<Vec<u8>, DecodeErrors> {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.set_cancel(cancel);
    decoder.decode()
}

fn decode_with_interval(
    data: &[u8], cancel: impl CancelCheck + 'static, interval: usize
) -> Result<Vec<u8>, DecodeErrors> {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.set_cancel(cancel);
    decoder.set_cancel_interval(interval);
    decoder.decode()
}

fn decode_with_repeated_cancellation(data: &[u8], polls_per_attempt: usize) -> Vec<u8> {
    const MAX_ATTEMPTS: usize = 128;

    let expected = JpegDecoder::new(ZCursor::new(data)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);

    let mut out = vec![0; decoder.output_buffer_size().unwrap()];
    let mut cancellations = 0;
    let mut stable_bytes = 0;

    for _ in 0..MAX_ATTEMPTS {
        decoder.set_cancel(cancel_after(polls_per_attempt));

        match decoder.decode_into(&mut out) {
            Ok(()) => {
                assert!(cancellations > 1, "fixture should require multiple resumptions");
                if let Some(index) = out.iter().zip(&expected).position(|(a, b)| a != b) {
                    panic!(
                        "resumed output first differs at byte {index}: actual={}, expected={}",
                        out[index], expected[index]
                    );
                }
                return out;
            }
            Err(DecodeErrors::Cancelled) => {
                cancellations += 1;
                let next_stable = decoder.decoded_output_bytes().unwrap();
                assert!(
                    next_stable >= stable_bytes,
                    "stable output must not move backwards across resumptions"
                );
                stable_bytes = next_stable;
            }
            Err(error) => panic!("unexpected decode error after {cancellations} yields: {error:?}")
        }
    }

    panic!("decode did not progress after {MAX_ATTEMPTS} cancellation resumptions");
}

fn decode_after_one_cancellation(data: &[u8], polls_before_cancel: usize) -> Vec<u8> {
    let expected = JpegDecoder::new(ZCursor::new(data)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);

    let mut out = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.set_cancel(cancel_after(polls_before_cancel));
    assert!(matches!(
        decoder.decode_into(&mut out),
        Err(DecodeErrors::Cancelled)
    ));

    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut out).unwrap();
    if let Some(index) = out.iter().zip(&expected).position(|(a, b)| a != b) {
        panic!(
            "output after cancellation at poll {polls_before_cancel} first differs at byte {index}: actual={}, expected={}",
            out[index], expected[index]
        );
    }
    out
}

#[test]
fn never_cancelling_matches_plain_decode() {
    for data in [BASELINE, PROGRESSIVE] {
        let plain = JpegDecoder::new(ZCursor::new(data)).decode().unwrap();
        let with_none = decode_with(data, NeverCancel).unwrap();
        let with_live = decode_with(data, cancel_after(usize::MAX)).unwrap();
        assert_eq!(plain, with_none);
        assert_eq!(plain, with_live);
    }
}

#[test]
fn incremental_progressive_without_cancellation_matches_plain_decode() {
    let expected = JpegDecoder::new(ZCursor::new(PROGRESSIVE)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(PROGRESSIVE));
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    let mut out = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut out).unwrap();
    assert_eq!(out, expected);
}

#[test]
fn cancelling_returns_cancelled() {
    for data in [BASELINE, PROGRESSIVE] {
        for polls in [0, 2] {
            let err = decode_with(data, cancel_after(polls)).unwrap_err();
            assert!(matches!(err, DecodeErrors::Cancelled));
        }
    }
}

#[test]
fn cancel_interval_round_trips_and_clamps() {
    let mut decoder = JpegDecoder::new(ZCursor::new(BASELINE));
    assert_eq!(decoder.cancel_interval(), 1024); // documented default
    decoder.set_cancel_interval(256);
    assert_eq!(decoder.cancel_interval(), 256);
    decoder.set_cancel_interval(0);
    assert_eq!(decoder.cancel_interval(), 1); // zero clamps to one
}

#[test]
fn interval_does_not_change_output() {
    // The poll interval is purely a debounce knob: it controls how often a live
    // (but never-firing) check is consulted, never the decoded pixels.
    for data in [BASELINE, PROGRESSIVE] {
        let plain = JpegDecoder::new(ZCursor::new(data)).decode().unwrap();
        for interval in [1, 7, 1024, usize::MAX] {
            let out = decode_with_interval(data, cancel_after(usize::MAX), interval).unwrap();
            assert_eq!(plain, out, "interval {interval} changed the decoded output");
        }
    }
}

#[test]
fn non_firing_progressive_cancel_does_not_query_position_per_row() {
    let position_queries = Arc::new(AtomicUsize::new(0));
    let reader = PositionCountingReader {
        inner: Cursor::new(PROGRESSIVE),
        position_queries: Arc::clone(&position_queries)
    };
    let mut decoder = JpegDecoder::new(reader);
    decoder.decode_headers().unwrap();
    let queries_after_headers = position_queries.load(Ordering::Relaxed);
    decoder.set_cancel(cancel_after(usize::MAX));
    decoder.set_cancel_interval(1);

    let mut out = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut out).unwrap();

    assert_eq!(
        position_queries.load(Ordering::Relaxed),
        queries_after_headers,
        "a non-firing cancel check must not query the reader position"
    );
}

#[test]
fn fine_interval_still_cancels() {
    // Polling as often as the loop allows (interval 1 -> once per MCU row) must
    // still surface a cancel on the very first poll.
    for data in [BASELINE, PROGRESSIVE] {
        let err = decode_with_interval(data, cancel_after(0), 1).unwrap_err();
        assert!(matches!(err, DecodeErrors::Cancelled));
    }
}

#[test]
fn baseline_decode_resumes_after_repeated_cancellation() {
    decode_with_repeated_cancellation(BASELINE, 32);
}

#[test]
fn progressive_decode_resumes_after_repeated_cancellation() {
    for data in [PROGRESSIVE, PROGRESSIVE_GRAYSCALE] {
        decode_with_repeated_cancellation(data, 32);
    }
}

#[test]
fn progressive_decode_resumes_after_one_cancellation() {
    for polls in [0, 1, 8, 32, 128, 512] {
        decode_after_one_cancellation(PROGRESSIVE, polls);
    }
}

#[test]
fn progressive_scan_limit_survives_repeated_cancellation() {
    let expected = JpegDecoder::new(ZCursor::new(PROGRESSIVE)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(PROGRESSIVE));
    decoder.set_options(decoder.options().jpeg_set_max_scans(8));
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    let mut out = vec![0; decoder.output_buffer_size().unwrap()];

    for _ in 0..128 {
        decoder.set_cancel(cancel_after(32));
        match decoder.decode_into(&mut out) {
            Err(DecodeErrors::Cancelled) => {}
            Err(DecodeErrors::Format(message)) => {
                assert!(message.contains("Too many scans"));
                decoder.set_options(decoder.options().jpeg_set_max_scans(100));
                decoder.set_cancel(NeverCancel);
                decoder.decode_into(&mut out).unwrap();
                assert_eq!(out, expected, "hard-error cleanup must permit exact replay");
                return;
            }
            Err(error) => panic!("unexpected decode error: {error:?}"),
            Ok(()) => panic!("cancellation must not bypass the progressive scan limit")
        }
    }

    panic!("decode did not reach the progressive scan limit");
}

#[test]
#[cfg(feature = "arith")]
fn progressive_arithmetic_replays_safely_after_cancellation() {
    decode_after_one_cancellation(PROGRESSIVE_ARITHMETIC, 32);
}
