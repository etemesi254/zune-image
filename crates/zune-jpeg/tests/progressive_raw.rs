/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under the terms of the MIT, Apache License or Zlib license
 */

//! Progressive raw component output coverage.

use std::cell::Cell;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_jpeg::{JpegDecoder, PlaneInfo, RawImcuRowStatus};

const PROGRESSIVE_420_WEBCODECS: &[u8] =
    include_bytes!("../../../test-images/jpeg/progressive_420_webcodecs.jpg");

struct GrowableCursor<'a> {
    data:     &'a [u8],
    position: usize,
    limit:    Rc<Cell<usize>>
}

impl<'a> GrowableCursor<'a> {
    fn new(data: &'a [u8], limit: Rc<Cell<usize>>) -> Self {
        Self {
            data,
            position: 0,
            limit
        }
    }

    fn visible(&self) -> usize {
        self.limit.get().min(self.data.len())
    }
}

impl Read for GrowableCursor<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let visible = self.visible();
        if self.position >= visible {
            return Ok(0);
        }
        let count = output.len().min(visible - self.position);
        output[..count].copy_from_slice(&self.data[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

impl BufRead for GrowableCursor<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let visible = self.visible();
        Ok(&self.data[self.position.min(visible)..visible])
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
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start"
            ));
        }
        self.position = next as usize;
        Ok(self.position as u64)
    }
}

fn decode_whole(bytes: &[u8]) -> (Vec<Vec<u8>>, [PlaneInfo; 4], usize) {
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let mut planes: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.byte_size])
        .collect();
    let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes(&mut refs).unwrap();
    (planes, layout, count)
}

fn assert_logical_planes_equal(
    actual: &[Vec<u8>], actual_strides: &[usize], expected: &[Vec<u8>], layout: &[PlaneInfo],
    count: usize
) {
    for index in 0..count {
        for row in 0..layout[index].height {
            assert_eq!(
                &actual[index][row * actual_strides[index]
                    ..row * actual_strides[index] + layout[index].width],
                &expected[index]
                    [row * layout[index].stride..row * layout[index].stride + layout[index].width],
                "component {index}, row {row}"
            );
        }
    }
}

fn assert_pull_matches_whole(bytes: &[u8]) {
    let (expected, layout, count) = decode_whole(bytes);
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    let mut actual: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.height])
        .collect();
    let mut rows = vec![0usize; count];

    loop {
        let mut stripe: Vec<Vec<u8>> = layout[..count]
            .iter()
            .map(|plane| vec![0xCD; plane.width * plane.vertical_sampling_factor * 8])
            .collect();
        let mut refs: Vec<&mut [u8]> = stripe.iter_mut().map(Vec::as_mut_slice).collect();
        match raw.decode_next_imcu_row(&mut refs, &strides).unwrap() {
            RawImcuRowStatus::RowReady { rows_written } => {
                for index in 0..count {
                    let len = rows_written[index] * strides[index];
                    let start = rows[index] * strides[index];
                    actual[index][start..start + len].copy_from_slice(&stripe[index][..len]);
                    assert!(stripe[index][len..].iter().all(|byte| *byte == 0xCD));
                    rows[index] += rows_written[index];
                }
            }
            RawImcuRowStatus::NeedMoreInput => panic!("complete input suspended"),
            RawImcuRowStatus::Complete => break,
            _ => unreachable!()
        }
    }

    for index in 0..count {
        assert_eq!(rows[index], layout[index].height);
    }
    assert_logical_planes_equal(&actual, &strides, &expected, &layout, count);
}

#[test]
fn progressive_420_webcodecs_layout_strides_and_padding() {
    // WPT `webcodecs/four-colors-limited-range-420-8bpc.jpg`.
    let (expected, layout, count) = decode_whole(PROGRESSIVE_420_WEBCODECS);
    assert_eq!(count, 3);
    assert_eq!(
        (
            layout[0].horizontal_sampling_factor,
            layout[0].vertical_sampling_factor,
            layout[0].width,
            layout[0].height
        ),
        (2, 2, 320, 240)
    );
    for plane in &layout[1..3] {
        assert_eq!(
            (
                plane.horizontal_sampling_factor,
                plane.vertical_sampling_factor,
                plane.width,
                plane.height
            ),
            (1, 1, 160, 120)
        );
    }

    let mut decoder = JpegDecoder::new(ZCursor::new(PROGRESSIVE_420_WEBCODECS));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    assert_eq!(raw.component_ids().as_deref(), Some(&[1, 2, 3][..]));
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 13)
        .collect();
    let mut actual: Vec<Vec<u8>> = layout[..count]
        .iter()
        .enumerate()
        .map(|(index, plane)| vec![0xCD; strides[index] * plane.height + 17])
        .collect();
    let mut refs: Vec<&mut [u8]> = actual.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes_strided(&mut refs, &strides).unwrap();

    assert_logical_planes_equal(&actual, &strides, &expected, &layout, count);
    for index in 0..count {
        for row in 0..layout[index].height {
            assert!(actual[index]
                [row * strides[index] + layout[index].width..(row + 1) * strides[index]]
                .iter()
                .all(|byte| *byte == 0xCD));
        }
        assert!(actual[index][strides[index] * layout[index].height..]
            .iter()
            .all(|byte| *byte == 0xCD));
    }
}

#[test]
fn progressive_yuv_sampling_modes_pull_match_whole_output() {
    for bytes in [
        PROGRESSIVE_420_WEBCODECS,
        include_bytes!("../../../test-images/jpeg/progressive_422_65x67.jpg").as_slice(),
        include_bytes!("../../../test-images/jpeg/progressive_444_65x67.jpg").as_slice()
    ] {
        assert_pull_matches_whole(bytes);
    }
}

#[test]
fn progressive_444_and_odd_422_layouts() {
    for (name, bytes, expected) in [
        (
            "444",
            include_bytes!("../../../test-images/jpeg/progressive_444_65x67.jpg").as_slice(),
            [(1, 1, 65, 67), (1, 1, 65, 67), (1, 1, 65, 67)]
        ),
        (
            "422",
            include_bytes!("../../../test-images/jpeg/progressive_422_65x67.jpg").as_slice(),
            [(2, 1, 65, 67), (1, 1, 33, 67), (1, 1, 33, 67)]
        )
    ] {
        let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
        decoder.decode_headers().unwrap();
        let raw = decoder.raw_output();
        assert_eq!(
            raw.component_ids().as_deref(),
            Some(&[1, 2, 3][..]),
            "{name}"
        );
        let layout = raw.layout().unwrap();
        for (index, (h, v, width, height)) in expected.into_iter().enumerate() {
            assert_eq!(
                (
                    layout[index].horizontal_sampling_factor,
                    layout[index].vertical_sampling_factor,
                    layout[index].width,
                    layout[index].height
                ),
                (h, v, width, height),
                "{name}, component {index}"
            );
        }
    }
}

#[test]
fn progressive_raw_retry_is_atomic_and_exact() {
    let (expected, layout, count) = decode_whole(PROGRESSIVE_420_WEBCODECS);
    let limit = Rc::new(Cell::new(PROGRESSIVE_420_WEBCODECS.len() / 2));
    let cursor = GrowableCursor::new(PROGRESSIVE_420_WEBCODECS, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 7)
        .collect();
    let mut actual: Vec<Vec<u8>> = layout[..count]
        .iter()
        .enumerate()
        .map(|(index, plane)| vec![0xCD; strides[index] * plane.height])
        .collect();
    let mut refs: Vec<&mut [u8]> = actual.iter_mut().map(Vec::as_mut_slice).collect();
    let error = raw
        .decode_into_planes_strided(&mut refs, &strides)
        .unwrap_err();
    assert!(error.is_recoverable_eof(), "{error:?}");
    assert!(actual
        .iter()
        .all(|plane| plane.iter().all(|byte| *byte == 0xCD)));

    limit.set(PROGRESSIVE_420_WEBCODECS.len());
    let mut refs: Vec<&mut [u8]> = actual.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes_strided(&mut refs, &strides).unwrap();
    assert_logical_planes_equal(&actual, &strides, &expected, &layout, count);
}

#[test]
fn progressive_raw_retries_exactly_at_every_scan_cutoff() {
    let (expected, layout, count) = decode_whole(PROGRESSIVE_420_WEBCODECS);
    let first_sos = PROGRESSIVE_420_WEBCODECS
        .windows(2)
        .position(|marker| marker == [0xFF, 0xDA])
        .unwrap();
    let sos_length = usize::from(u16::from_be_bytes([
        PROGRESSIVE_420_WEBCODECS[first_sos + 2],
        PROGRESSIVE_420_WEBCODECS[first_sos + 3]
    ]));
    let scan_start = first_sos + 2 + sos_length;

    for cutoff in scan_start..PROGRESSIVE_420_WEBCODECS.len() {
        let limit = Rc::new(Cell::new(cutoff));
        let cursor = GrowableCursor::new(PROGRESSIVE_420_WEBCODECS, Rc::clone(&limit));
        let mut decoder = JpegDecoder::new(cursor);
        decoder.decode_headers().unwrap();
        let mut raw = decoder.raw_output();
        let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width + 3).collect();
        let mut actual: Vec<Vec<u8>> = layout[..count]
            .iter()
            .enumerate()
            .map(|(index, plane)| vec![0xCD; strides[index] * plane.height])
            .collect();
        let mut refs: Vec<&mut [u8]> = actual.iter_mut().map(Vec::as_mut_slice).collect();
        match raw.decode_into_planes_strided(&mut refs, &strides) {
            Ok(()) => {}
            Err(error) if error.is_recoverable_eof() => {
                assert!(
                    actual
                        .iter()
                        .all(|plane| plane.iter().all(|byte| *byte == 0xCD)),
                    "cutoff {cutoff} modified caller planes"
                );
                limit.set(PROGRESSIVE_420_WEBCODECS.len());
                let mut refs: Vec<&mut [u8]> =
                    actual.iter_mut().map(Vec::as_mut_slice).collect();
                raw.decode_into_planes_strided(&mut refs, &strides)
                    .unwrap();
            }
            Err(error) => panic!("cutoff {cutoff}: {error:?}")
        }
        assert_logical_planes_equal(&actual, &strides, &expected, &layout, count);
    }
}

#[test]
fn progressive_raw_cancellation_discards_partial_scans_and_retries() {
    let (expected, layout, count) = decode_whole(PROGRESSIVE_420_WEBCODECS);
    let polls = Arc::new(AtomicUsize::new(0));
    let cancel_at = Arc::new(AtomicUsize::new(10));
    let callback_polls = Arc::clone(&polls);
    let callback_cancel_at = Arc::clone(&cancel_at);
    let mut decoder = JpegDecoder::new(ZCursor::new(PROGRESSIVE_420_WEBCODECS));
    decoder.decode_headers().unwrap();
    decoder.set_cancel(move || {
        callback_polls.fetch_add(1, Ordering::SeqCst)
            >= callback_cancel_at.load(Ordering::SeqCst)
    });
    decoder.set_cancel_interval(1);
    let mut raw = decoder.raw_output();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width + 5).collect();
    let mut actual: Vec<Vec<u8>> = layout[..count]
        .iter()
        .enumerate()
        .map(|(index, plane)| vec![0xCD; strides[index] * plane.height])
        .collect();
    let mut refs: Vec<&mut [u8]> = actual.iter_mut().map(Vec::as_mut_slice).collect();
    assert!(matches!(
        raw.decode_into_planes_strided(&mut refs, &strides),
        Err(zune_jpeg::errors::DecodeErrors::Cancelled)
    ));
    assert!(polls.load(Ordering::SeqCst) > 10);
    assert!(
        actual
            .iter()
            .all(|plane| plane.iter().all(|byte| *byte == 0xCD))
    );

    cancel_at.store(usize::MAX, Ordering::SeqCst);
    let mut refs: Vec<&mut [u8]> = actual.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes_strided(&mut refs, &strides)
        .unwrap();
    assert_logical_planes_equal(&actual, &strides, &expected, &layout, count);
}

#[test]
fn malformed_progressive_scan_does_not_publish_raw_planes() {
    let mut malformed = PROGRESSIVE_420_WEBCODECS.to_vec();
    let sos: Vec<_> = malformed
        .windows(2)
        .enumerate()
        .filter_map(|(offset, marker)| (marker == [0xFF, 0xDA]).then_some(offset))
        .collect();
    assert!(sos.len() > 1);
    let second_sos = sos[1];
    let components = usize::from(malformed[second_sos + 4]);
    malformed[second_sos + 5 + 2 * components] = 64;

    let options = DecoderOptions::default().set_strict_mode(true);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&malformed), options);
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let mut planes: Vec<Vec<u8>> = layout[..3]
        .iter()
        .map(|plane| vec![0xCD; plane.byte_size])
        .collect();
    let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    let error = raw.decode_into_planes(&mut refs).unwrap_err();
    assert!(!error.is_recoverable_eof(), "{error:?}");
    assert!(planes
        .iter()
        .all(|plane| plane.iter().all(|byte| *byte == 0xCD)));
}

#[test]
fn progressive_raw_and_packed_output_replay() {
    let expected_packed = JpegDecoder::new(ZCursor::new(PROGRESSIVE_420_WEBCODECS))
        .decode()
        .unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(PROGRESSIVE_420_WEBCODECS));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let mut first: Vec<Vec<u8>> = layout[..3]
        .iter()
        .map(|plane| vec![0; plane.byte_size])
        .collect();
    let mut refs: Vec<&mut [u8]> = first.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes(&mut refs).unwrap();

    let mut replay: Vec<Vec<u8>> = layout[..3]
        .iter()
        .map(|plane| vec![0xCD; plane.byte_size])
        .collect();
    let mut refs: Vec<&mut [u8]> = replay.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes(&mut refs).unwrap();
    assert_eq!(replay, first);
    drop(raw);

    assert_eq!(decoder.decode().unwrap(), expected_packed);
}
