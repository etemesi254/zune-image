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
