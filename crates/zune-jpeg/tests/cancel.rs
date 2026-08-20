/*
 * Copyright (c) 2026.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{cell::{Cell, RefCell}, io::{BufRead, Read, Seek, SeekFrom}, rc::Rc};

use zune_core::bytestream::ZCursor;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{CancelCheck, JpegDecoder, NeverCancel};

const BASELINE: &[u8] = include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg");
const PROGRESSIVE: &[u8] =
    include_bytes!("../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg");
const SMALL_PROGRESSIVE: &[u8] =
    include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg");
const MULTI_SOS: &[u8] = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
const RESTART: &[u8] = include_bytes!("../../../test-images/jpeg/four_components.jpg");
const PROGRESSIVE_RESTART: &[u8] =
    include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg");

struct GrowableCursor<'a> {
    data:              &'a [u8],
    position:          usize,
    limit:             Rc<Cell<usize>>,
    seek_log:          Option<Rc<RefCell<Vec<usize>>>>,
    position_observer: Option<Arc<AtomicUsize>>
}

impl<'a> GrowableCursor<'a> {
    fn new(data: &'a [u8], limit: Rc<Cell<usize>>) -> Self {
        Self { data, position: 0, limit, seek_log: None, position_observer: None }
    }

    fn with_seek_log(
        data: &'a [u8], limit: Rc<Cell<usize>>, seek_log: Rc<RefCell<Vec<usize>>>
    ) -> Self {
        Self { data, position: 0, limit, seek_log: Some(seek_log), position_observer: None }
    }

    fn with_position_observer(
        data: &'a [u8], limit: Rc<Cell<usize>>, position_observer: Arc<AtomicUsize>
    ) -> Self {
        Self { data, position: 0, limit, seek_log: None, position_observer: Some(position_observer) }
    }

    fn visible(&self) -> usize {
        self.limit.get().min(self.data.len())
    }

    fn set_position(&mut self, position: usize) {
        self.position = position;
        if let Some(observer) = &self.position_observer {
            observer.store(position, Ordering::Relaxed);
        }
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
        self.set_position(self.position + count);
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
        self.set_position(self.position + amount);
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
        self.set_position(next as usize);
        if let Some(seek_log) = &self.seek_log {
            seek_log.borrow_mut().push(self.position);
        }
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

fn sos_data_start(data: &[u8], index: usize) -> usize {
    let offset = sos_offset(data, index);
    let length = usize::from(u16::from_be_bytes([data[offset + 2], data[offset + 3]]));
    offset + 2 + length
}

fn ac_first_scan_start(data: &[u8], index: usize) -> usize {
    let sos = sos_offset(data, index);
    let scan_components = usize::from(data[sos + 4]);
    let spectral_params = sos + 5 + scan_components * 2;
    assert!(data[spectral_params] > 0, "scan must be AC");
    assert_eq!(data[spectral_params + 2] >> 4, 0, "scan must be AC first");
    sos_data_start(data, index)
}

fn icc_app2(sequence: u8, total: u8, payload: &[u8]) -> Vec<u8> {
    let body_len = 12 + 2 + payload.len();
    let length = u16::try_from(body_len + 2).unwrap().to_be_bytes();
    let mut marker = vec![0xFF, 0xE2, length[0], length[1]];
    marker.extend_from_slice(b"ICC_PROFILE\0");
    marker.extend_from_slice(&[sequence, total]);
    marker.extend_from_slice(payload);
    marker
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
fn header_marker_cancellation_retries() {
    let expected = JpegDecoder::new(ZCursor::new(PROGRESSIVE)).decode().unwrap();
    for cancel_poll in [0, 2] {
        let mut decoder = JpegDecoder::new(ZCursor::new(PROGRESSIVE));
        decoder.set_cancel(cancel_once_at(cancel_poll));
        let error = decoder.decode_headers().unwrap_err();
        assert!(matches!(error, DecodeErrors::Cancelled));

        decoder.set_cancel(NeverCancel);
        decoder.decode_headers().unwrap();
        let mut output = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.decode_into(&mut output).unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn unknown_header_marker_cancellation_retries() {
    let mut data = PROGRESSIVE.to_vec();
    // Insert an unknown length-prefixed marker immediately after SOI.
    data.splice(2..2, [0xFF, 0xF0, 0x00, 0x06, 1, 2, 3, 4]);
    let expected = JpegDecoder::new(ZCursor::new(&data)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    decoder.set_cancel(cancel_once_at(1));

    let error = decoder.decode_headers().unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    decoder.set_cancel(NeverCancel);
    decoder.decode_headers().unwrap();
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn large_marker_payload_cancellation_retries() {
    let mut data = PROGRESSIVE.to_vec();
    let payload = vec![0x5A; 8192];
    let length = u16::try_from(payload.len() + 2).unwrap().to_be_bytes();
    let mut marker = vec![0xFF, 0xEF, length[0], length[1]];
    marker.extend_from_slice(&payload);
    data.splice(2..2, marker);

    let expected = JpegDecoder::new(ZCursor::new(&data)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    // Start-of-call and APP dispatch are the first two polls; the third occurs
    // between 4 KiB marker-body chunks.
    decoder.set_cancel(cancel_once_at(2));
    let error = decoder.decode_headers().unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));

    decoder.set_cancel(NeverCancel);
    decoder.decode_headers().unwrap();
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn metadata_marker_cancellation_commits_once() {
    let first_payload = b"first-icc-segment";
    let second_payload = b"second-icc-segment";
    let mut data = SMALL_PROGRESSIVE.to_vec();
    let mut markers = icc_app2(1, 2, first_payload);
    markers.extend_from_slice(&icc_app2(2, 2, second_payload));
    data.splice(2..2, markers);

    let expected = JpegDecoder::new(ZCursor::new(&data)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    // Cancel before the second ICC segment after the first has committed and
    // advanced the header checkpoint.
    decoder.set_cancel(cancel_once_at(2));
    let error = decoder.decode_headers().unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));

    decoder.set_cancel(NeverCancel);
    decoder.decode_headers().unwrap();
    let mut expected_icc = first_payload.to_vec();
    expected_icc.extend_from_slice(second_payload);
    assert_eq!(decoder.icc_profile().as_deref(), Some(expected_icc.as_slice()));
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn progressive_fine_cancellation_resume_uses_checkpoint() {
    let expected = JpegDecoder::new(ZCursor::new(PROGRESSIVE_RESTART)).decode().unwrap();
    let first_scan_start = sos_data_start(PROGRESSIVE_RESTART, 0);
    let second_sos = sos_offset(PROGRESSIVE_RESTART, 1);
    assert!(
        PROGRESSIVE_RESTART[first_scan_start..second_sos]
            .windows(2)
            .any(|bytes| bytes[0] == 0xFF && (0xD0..=0xD7).contains(&bytes[1])),
        "fixture must contain RST markers in the first DC scan"
    );

    let limit = Rc::new(Cell::new(PROGRESSIVE_RESTART.len()));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(
        PROGRESSIVE_RESTART,
        Rc::clone(&limit),
        Rc::clone(&seek_log)
    );
    let mut decoder = JpegDecoder::new(cursor);
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    decoder.set_cancel(cancel_once_at(8));
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    seek_log.borrow_mut().clear();
    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);

    let first_retry_seek = seek_log.borrow()[0];
    assert!(
        first_retry_seek > first_scan_start && first_retry_seek < second_sos,
        "retry should resume inside the restart-bearing first DC scan; start={first_scan_start}, seek={first_retry_seek}, next_sos={second_sos}"
    );
}

#[test]
fn progressive_inter_scan_marker_cancellation_retries() {
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
fn progressive_unsafe_scan_cancellation_retries() {
    let expected = JpegDecoder::new(ZCursor::new(SMALL_PROGRESSIVE)).decode().unwrap();
    let unsafe_scan_start = ac_first_scan_start(SMALL_PROGRESSIVE, 1);
    let limit = Rc::new(Cell::new(SMALL_PROGRESSIVE.len()));
    let position = Arc::new(AtomicUsize::new(0));
    let cursor = GrowableCursor::with_position_observer(
        SMALL_PROGRESSIVE,
        Rc::clone(&limit),
        Arc::clone(&position)
    );
    let mut decoder = JpegDecoder::new(cursor);
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    decoder.set_cancel({
        let position = Arc::clone(&position);
        move || position.load(Ordering::Relaxed) > unsafe_scan_start
    });
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert!(
        position.load(Ordering::Relaxed) > unsafe_scan_start,
        "cancellation must occur after an AC-first row consumed entropy data"
    );
    assert_eq!(decoder.decoded_scans(), Some(1));

    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn progressive_non_incremental_unsafe_scan_cancellation_retries() {
    let expected = JpegDecoder::new(ZCursor::new(SMALL_PROGRESSIVE)).decode().unwrap();
    let unsafe_scan_start = ac_first_scan_start(SMALL_PROGRESSIVE, 1);
    let limit = Rc::new(Cell::new(SMALL_PROGRESSIVE.len()));
    let position = Arc::new(AtomicUsize::new(0));
    let cursor = GrowableCursor::with_position_observer(
        SMALL_PROGRESSIVE,
        Rc::clone(&limit),
        Arc::clone(&position)
    );
    let mut decoder = JpegDecoder::new(cursor);
    decoder.decode_headers().unwrap();
    decoder.set_cancel_interval(1);
    decoder.set_cancel({
        let position = Arc::clone(&position);
        move || position.load(Ordering::Relaxed) > unsafe_scan_start
    });
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert!(!error.is_recoverable_eof());
    assert!(
        position.load(Ordering::Relaxed) > unsafe_scan_start,
        "cancellation must occur after an AC-first row consumed entropy data"
    );
    assert_eq!(decoder.decoded_scans(), Some(0));
    assert_eq!(decoder.decoded_output_bytes(), Some(0));
    assert_eq!(decoder.decoded_scanlines(), Some(0));
    assert_eq!(decoder.decoded_preview_output_bytes(), Some(0));
    assert_eq!(decoder.decoded_preview_scanlines(), Some(0));
    assert!(output.iter().all(|byte| *byte == 0));

    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn baseline_cancellation_preserves_stable_progress_and_retries() {
    let data = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let expected = JpegDecoder::new(ZCursor::new(data)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
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
    let entropy_start = sos_data_start(RESTART, 0);
    let limit = Rc::new(Cell::new(RESTART.len()));
    let seek_log = Rc::new(RefCell::new(Vec::new()));
    let cursor = GrowableCursor::with_seek_log(RESTART, Rc::clone(&limit), Rc::clone(&seek_log));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.decode_headers().unwrap();
    decoder.set_incremental_mode(true);
    decoder.set_cancel_interval(1);
    decoder.set_cancel(cancel_once_at(2));
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    seek_log.borrow_mut().clear();
    decoder.set_cancel(NeverCancel);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output, expected);
    assert!(
        seek_log.borrow()[0] > entropy_start,
        "restart cancellation should resume from an in-scan checkpoint"
    );
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
