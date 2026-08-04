/*
 * Copyright (c) 2026.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{cell::Cell, io::{BufRead, Read, Seek, SeekFrom}, rc::Rc};

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{CancelCheck, JpegDecoder, NeverCancel};

const BASELINE: &[u8] = include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg");
const PROGRESSIVE: &[u8] =
    include_bytes!("../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg");
const SMALL_PROGRESSIVE: &[u8] =
    include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg");
const MULTI_SOS: &[u8] = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
const RESTART: &[u8] = include_bytes!("../../../test-images/jpeg/four_components.jpg");

struct GrowableCursor<'a> {
    data: &'a [u8],
    position: usize,
    limit: Rc<Cell<usize>>
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
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let visible = self.visible();
        if self.position >= visible {
            return Ok(0);
        }
        let available = &self.data[self.position..visible];
        let count = available.len().min(buffer.len());
        buffer[..count].copy_from_slice(&available[..count]);
        self.position += count;
        Ok(count)
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

    fn consume(&mut self, amount: usize) {
        self.position += amount;
    }
}

impl Seek for GrowableCursor<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let next = match position {
            SeekFrom::Start(value) => value as i64,
            SeekFrom::Current(value) => self.position as i64 + value,
            SeekFrom::End(value) => self.visible() as i64 + value
        };
        if next < 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before start"));
        }
        self.position = next as usize;
        Ok(self.position as u64)
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

fn cancel_once_at(n: usize) -> impl Fn() -> bool + Send + Sync {
    let polls = Arc::new(AtomicUsize::new(0));
    move || polls.fetch_add(1, Ordering::Relaxed) == n
}

fn count_decode_polls(data: &'static [u8], incremental: bool) -> usize {
    let polls = Arc::new(AtomicUsize::new(0));
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(incremental);
    decoder.set_cancel_interval(1);
    decoder.set_cancel({
        let polls = Arc::clone(&polls);
        move || {
            polls.fetch_add(1, Ordering::Relaxed);
            false
        }
    });
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut output).unwrap();
    polls.load(Ordering::Relaxed)
}

fn sos_offset(data: &[u8], index: usize) -> usize {
    data.windows(2)
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == [0xFF, 0xDA]).then_some(offset))
        .nth(index)
        .expect("progressive fixture must contain the requested SOS")
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
fn fine_interval_still_cancels() {
    // Polling as often as the loop allows (interval 1 -> once per MCU row) must
    // still surface a cancel on the very first poll.
    for data in [BASELINE, PROGRESSIVE] {
        let err = decode_with_interval(data, cancel_after(0), 1).unwrap_err();
        assert!(matches!(err, DecodeErrors::Cancelled));
    }
}

#[test]
fn progressive_edge_trigger_cancellation_is_reported() {
    for strict in [false, true] {
        for incremental in [false, true] {
            let options = DecoderOptions::default().set_strict_mode(strict);
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(PROGRESSIVE), options);
            decoder.decode_headers().unwrap();
            decoder.set_incremental_mode(incremental);
            decoder.set_cancel_interval(1);
            decoder.set_cancel(cancel_once_at(0));
            let mut output = vec![0; decoder.output_buffer_size().unwrap()];

            let error = decoder.decode_into(&mut output).unwrap_err();
            assert!(matches!(error, DecodeErrors::Cancelled));
        }
    }
}

#[test]
fn progressive_cancellation_retry_matches_oneshot() {
    let expected = JpegDecoder::new(ZCursor::new(PROGRESSIVE)).decode().unwrap();

    // Poll 8 cancels immediately after the first eligible eight-row fine
    // checkpoint, proving its matching scratch coefficients survive retry.
    for (incremental, cancel_poll) in [(false, 0), (true, 8)] {
        let mut decoder = JpegDecoder::new(ZCursor::new(PROGRESSIVE));
        decoder.decode_headers().unwrap();
        decoder.set_incremental_mode(incremental);
        decoder.set_cancel_interval(1);
        decoder.set_cancel(cancel_once_at(cancel_poll));
        let mut output = vec![0; decoder.output_buffer_size().unwrap()];

        let error = decoder.decode_into(&mut output).unwrap_err();
        assert!(matches!(error, DecodeErrors::Cancelled));

        decoder.set_cancel(NeverCancel);
        decoder.decode_into(&mut output).unwrap();
        assert_eq!(output, expected, "incremental={incremental}");
    }
}

#[test]
fn progressive_unsafe_scan_cancellation_retries() {
    let expected = JpegDecoder::new(ZCursor::new(SMALL_PROGRESSIVE)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(SMALL_PROGRESSIVE));
    decoder.decode_headers().unwrap();
    let first_scan_rows = usize::from(decoder.info().unwrap().height).div_ceil(8);
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    decoder.set_cancel(cancel_once_at(first_scan_rows));
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert_eq!(decoder.decoded_scans(), Some(1));

    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn baseline_cancellation_preserves_stable_progress_and_retries() {
    let expected = JpegDecoder::new(ZCursor::new(BASELINE)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(BASELINE));
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    decoder.set_cancel(cancel_once_at(2));
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    let stable_bytes = decoder.decoded_output_bytes().unwrap();
    let stable_scanlines = decoder.decoded_scanlines().unwrap();
    assert!(stable_bytes > 0);
    assert!(stable_scanlines > 0);

    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
    assert!(decoder.decoded_output_bytes().unwrap() >= stable_bytes);
    assert!(decoder.decoded_scanlines().unwrap() >= stable_scanlines);
}

#[test]
fn baseline_multi_sos_final_assembly_cancellation_retries() {
    let expected = JpegDecoder::new(ZCursor::new(MULTI_SOS)).decode().unwrap();
    let poll_count = count_decode_polls(MULTI_SOS, true);
    let mut decoder = JpegDecoder::new(ZCursor::new(MULTI_SOS));
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    decoder.set_cancel(cancel_once_at(poll_count - 1));
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert_eq!(decoder.decoded_output_bytes(), Some(0));

    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn baseline_restart_cancellation_retries() {
    let expected = JpegDecoder::new(ZCursor::new(RESTART)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(RESTART));
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    decoder.set_cancel(cancel_once_at(2));
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

fn decode_until_preview(
    cancel: impl CancelCheck + 'static
) -> (JpegDecoder<GrowableCursor<'static>>, Rc<Cell<usize>>, Vec<u8>) {
    let cutoff = sos_offset(SMALL_PROGRESSIVE, 1) + 1;
    let limit = Rc::new(Cell::new(cutoff));
    let cursor = GrowableCursor::new(SMALL_PROGRESSIVE, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    decoder.set_cancel(cancel);
    let mut output: Vec<u8> = vec![0; decoder.output_buffer_size().unwrap()];
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(error.is_recoverable_eof());
    assert!(decoder.decoded_preview_output_bytes().unwrap() > 0);
    (decoder, limit, output)
}

#[test]
fn progressive_render_cancellation_hides_partial_preview_and_retries() {
    let expected = JpegDecoder::new(ZCursor::new(SMALL_PROGRESSIVE)).decode().unwrap();

    let polls = Arc::new(AtomicUsize::new(0));
    let (mut reference, reference_limit, mut reference_output) = decode_until_preview({
        let polls = Arc::clone(&polls);
        move || {
            polls.fetch_add(1, Ordering::Relaxed);
            false
        }
    });
    polls.store(0, Ordering::Relaxed);
    reference_limit.set(SMALL_PROGRESSIVE.len());
    reference.decode_into(&mut reference_output).unwrap();
    // The last poll of this equivalent retry is in full-frame rendering,
    // after every entropy scan has committed.
    let final_retry_polls = polls.load(Ordering::Relaxed);
    assert!(final_retry_polls > 0);

    let (mut decoder, limit, mut output) = decode_until_preview(NeverCancel);
    limit.set(SMALL_PROGRESSIVE.len());
    decoder.set_cancel(cancel_once_at(final_retry_polls - 1));
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert_eq!(decoder.decoded_preview_output_bytes(), Some(0));
    assert_eq!(decoder.decoded_preview_scanlines(), Some(0));

    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn cancelled_preview_replacement_is_rerendered_before_publication() {
    let expected = JpegDecoder::new(ZCursor::new(SMALL_PROGRESSIVE)).decode().unwrap();
    let next_preview_cutoff = sos_offset(SMALL_PROGRESSIVE, 2) + 1;

    let polls = Arc::new(AtomicUsize::new(0));
    let (mut reference, reference_limit, mut reference_output) = decode_until_preview({
        let polls = Arc::clone(&polls);
        move || {
            polls.fetch_add(1, Ordering::Relaxed);
            false
        }
    });
    polls.store(0, Ordering::Relaxed);
    reference_limit.set(next_preview_cutoff);
    let error = reference.decode_into(&mut reference_output).unwrap_err();
    assert!(error.is_recoverable_eof());
    // The final poll replaces the existing preview with the newly completed
    // scan, so cancelling there creates the mixed-frame state under test.
    let replacement_polls = polls.load(Ordering::Relaxed);
    assert!(replacement_polls > 0);
    assert!(reference.decoded_scans().unwrap() >= 2);

    let (mut decoder, limit, mut output) = decode_until_preview(NeverCancel);
    limit.set(next_preview_cutoff);
    decoder.set_cancel(cancel_once_at(replacement_polls - 1));
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert_eq!(decoder.decoded_preview_output_bytes(), Some(0));

    decoder.set_cancel(NeverCancel);
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(error.is_recoverable_eof());
    assert!(decoder.decoded_scans().unwrap() >= 2);
    assert_eq!(decoder.decoded_preview_output_bytes(), Some(output.len()));

    limit.set(SMALL_PROGRESSIVE.len());
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
#[cfg(feature = "arith")]
fn arithmetic_cancellation_retries() {
    for data in [
        include_bytes!("../../../test-images/jpeg/arith/seq.jpg").as_slice(),
        include_bytes!("../../../test-images/jpeg/arith/prog.jpg").as_slice()
    ] {
        let expected = JpegDecoder::new(ZCursor::new(data)).decode().unwrap();
        for cancel_poll in [0, 2] {
            let mut decoder = JpegDecoder::new(ZCursor::new(data));
            decoder.decode_headers().unwrap();
            decoder.set_incremental_mode(true);
            decoder.set_cancel_interval(1);
            decoder.set_cancel(cancel_once_at(cancel_poll));
            let mut output = vec![0; decoder.output_buffer_size().unwrap()];

            let error = decoder.decode_into(&mut output).unwrap_err();
            assert!(matches!(error, DecodeErrors::Cancelled));
            decoder.set_cancel(NeverCancel);
            decoder.decode_into(&mut output).unwrap();
            assert_eq!(output, expected);
        }
    }
}
