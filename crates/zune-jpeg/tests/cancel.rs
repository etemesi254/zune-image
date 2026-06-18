/*
 * Copyright (c) 2026.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use zune_core::bytestream::ZCursor;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{CancelCheck, JpegDecoder, NeverCancel};

const BASELINE: &[u8] = include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg");
const PROGRESSIVE: &[u8] =
    include_bytes!("../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg");

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
