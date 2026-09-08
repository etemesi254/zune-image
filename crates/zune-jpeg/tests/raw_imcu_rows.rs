/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::cell::Cell;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{JpegDecoder, RawImcuRowStatus};

struct GrowableCursor<'a> {
    data: &'a [u8],
    position: usize,
    limit: Rc<Cell<usize>>,
}

impl<'a> GrowableCursor<'a> {
    fn new(data: &'a [u8], limit: Rc<Cell<usize>>) -> Self {
        Self {
            data,
            position: 0,
            limit,
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
            SeekFrom::End(value) => self.visible() as i64 + value,
        };
        if next < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.position = next as usize;
        Ok(self.position as u64)
    }
}

fn decode_whole(bytes: &[u8]) -> (Vec<Vec<u8>>, [zune_jpeg::PlaneInfo; 4], usize) {
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let mut planes: Vec<Vec<u8>> = (0..count)
        .map(|index| vec![0; layout[index].byte_size])
        .collect();
    let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into(&mut refs).unwrap();
    (planes, layout, count)
}

fn assert_pull_matches_whole(bytes: &[u8]) {
    let (whole, layout, count) = decode_whole(bytes);
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let mut actual: Vec<Vec<u8>> = (0..count)
        .map(|index| vec![0; layout[index].width * layout[index].height])
        .collect();
    let mut component_rows = [0usize; 4];

    loop {
        let strides: Vec<usize> = (0..count).map(|index| layout[index].width).collect();
        let mut stripe_storage: Vec<Vec<u8>> = (0..count)
            .map(|index| {
                let max_rows = layout[index].vertical_sampling_factor * 8;
                vec![0xCD; strides[index] * max_rows]
            })
            .collect();
        let mut refs: Vec<&mut [u8]> = stripe_storage.iter_mut().map(Vec::as_mut_slice).collect();

        match raw.decode_next_imcu_row(&mut refs, &strides).unwrap() {
            RawImcuRowStatus::RowReady { rows_written } => {
                for index in 0..count {
                    let row_start = component_rows[index];
                    let row_end = row_start + rows_written[index];
                    let dst_start = row_start * strides[index];
                    let dst_end = row_end * strides[index];
                    actual[index][dst_start..dst_end].copy_from_slice(
                        &stripe_storage[index][..rows_written[index] * strides[index]]
                    );
                    assert!(
                        stripe_storage[index][rows_written[index] * strides[index]..]
                            .iter()
                            .all(|byte| *byte == 0xCD)
                    );
                    component_rows[index] = row_end;
                }
            }
            RawImcuRowStatus::NeedMoreInput => panic!("one-shot input unexpectedly suspended"),
            RawImcuRowStatus::Complete => break,
            _ => unreachable!("unknown raw iMCU-row status")
        }
    }

    for index in 0..count {
        assert_eq!(component_rows[index], layout[index].height);
        for row in 0..layout[index].height {
            let actual_row =
                &actual[index][row * layout[index].width..(row + 1) * layout[index].width];
            let whole_row = &whole[index]
                [row * layout[index].stride..row * layout[index].stride + layout[index].width];
            assert_eq!(actual_row, whole_row, "component {index}, row {row}");
        }
    }
}

#[test]
fn pull_rows_match_whole_image_420() {
    assert_pull_matches_whole(include_bytes!("../../../test-images/jpeg/2029.jpg"));
}

#[test]
fn pull_rows_match_whole_image_444() {
    assert_pull_matches_whole(include_bytes!(
        "../../../test-images/jpeg/non_interleaved_444_64x64.jpg"
    ));
}

#[test]
fn pull_rows_match_whole_image_422() {
    assert_pull_matches_whole(include_bytes!(
        "../../../test-images/jpeg/non_interleaved_422_64x64.jpg"
    ));
}

#[test]
fn pull_rows_match_whole_image_odd_dimensions() {
    assert_pull_matches_whole(include_bytes!(
        "../../../test-images/jpeg/non_interleaved_422_65x65.jpg"
    ));
}

#[test]
fn pull_rows_match_whole_image_progressive() {
    assert_pull_matches_whole(include_bytes!(
        "../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"
    ));
}

#[test]
fn pull_rows_match_whole_image_non_interleaved_420() {
    assert_pull_matches_whole(include_bytes!(
        "../../../test-images/jpeg/non_interleaved_420_64x64.jpg"
    ));
}

#[test]
fn pull_rows_match_whole_image_440() {
    assert_pull_matches_whole(include_bytes!(
        "../../../test-images/jpeg/non_interleaved_440_64x64.jpg"
    ));
}

#[test]
fn pull_rows_match_whole_image_410() {
    assert_pull_matches_whole(include_bytes!("../../../test-images/jpeg/fox410.jpg"));
}

#[test]
fn pull_rows_support_custom_strides_and_report_component_rows() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 13)
        .collect();
    let mut storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .enumerate()
        .map(|(index, plane)| vec![0xCD; strides[index] * plane.vertical_sampling_factor * 8])
        .collect();
    let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();

    let rows_written = match raw.decode_next_imcu_row(&mut refs, &strides).unwrap() {
        RawImcuRowStatus::RowReady { rows_written } => rows_written,
        _ => panic!("first iMCU row was not ready")
    };
    assert_eq!(rows_written[0], 16);
    assert_eq!(rows_written[1], 8);
    assert_eq!(rows_written[2], 8);
    for index in 0..count {
        for row in 0..rows_written[index] {
            let padding = &storage[index]
                [row * strides[index] + layout[index].width..(row + 1) * strides[index]];
            assert!(padding.iter().all(|byte| *byte == 0xCD));
        }
    }
}

fn pull_required_len(rows: usize, stride: usize, width: usize) -> usize {
    if rows == 0 {
        0
    } else {
        (rows - 1) * stride + width
    }
}

fn with_luma_sampling(data: &[u8], sampling: u8) -> Vec<u8> {
    let mut bytes = data.to_vec();
    let sof = bytes
        .windows(2)
        .position(|marker| marker == [0xff, 0xc0])
        .expect("baseline SOF marker");
    bytes[sof + 11] = sampling;
    bytes
}

fn assert_exact_sized_pull_matches_whole(bytes: &[u8]) {
    const GUARD: usize = 17;
    const SENTINEL: u8 = 0xCD;

    let (whole, layout, count) = decode_whole(bytes);
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 13)
        .collect();
    let mut component_rows = [0usize; 4];

    loop {
        let rows: Vec<usize> = layout[..count]
            .iter()
            .enumerate()
            .map(|(index, plane)| {
                (plane.height - component_rows[index]).min(plane.vertical_sampling_factor * 8)
            })
            .collect();
        let required: Vec<usize> = layout[..count]
            .iter()
            .enumerate()
            .map(|(index, plane)| pull_required_len(rows[index], strides[index], plane.width))
            .collect();
        let mut storage: Vec<Vec<u8>> = required
            .iter()
            .map(|length| vec![SENTINEL; length + GUARD])
            .collect();
        let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();

        match raw.decode_next_imcu_row(&mut refs, &strides).unwrap() {
            RawImcuRowStatus::RowReady { rows_written } => {
                for index in 0..count {
                    assert_eq!(rows_written[index], rows[index]);
                    for row in 0..rows[index] {
                        let actual_start = row * strides[index];
                        let expected_start = (component_rows[index] + row) * layout[index].stride;
                        assert_eq!(
                            &storage[index][actual_start..actual_start + layout[index].width],
                            &whole[index][expected_start..expected_start + layout[index].width]
                        );
                        if row + 1 < rows[index] {
                            assert!(storage[index]
                                [actual_start + layout[index].width..(row + 1) * strides[index]]
                                .iter()
                                .all(|byte| *byte == SENTINEL));
                        }
                    }
                    assert!(storage[index][required[index]..]
                        .iter()
                        .all(|byte| *byte == SENTINEL));
                    component_rows[index] += rows_written[index];
                }
            }
            RawImcuRowStatus::Complete => break,
            RawImcuRowStatus::NeedMoreInput => panic!("one-shot input suspended"),
            _ => unreachable!("unknown raw iMCU-row status"),
        }
    }

    for index in 0..count {
        assert_eq!(component_rows[index], layout[index].height);
    }
}

fn assert_incremental_pull_is_atomic(
    name: &str, data: &[u8], step: usize, minimum_suspensions: usize,
) {
    const GUARD: usize = 17;
    const SENTINEL: u8 = 0xCD;

    let (whole, _, _) = decode_whole(data);
    let limit = Rc::new(Cell::new(0));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    let mut available = 0;
    loop {
        available = (available + step).min(data.len());
        limit.set(available);
        match decoder.decode_headers() {
            Ok(()) => break,
            Err(ref error) if error.is_recoverable_eof() => {
                assert!(
                    available < data.len(),
                    "{name}: headers suspended with full input"
                );
            }
            Err(error) => panic!("{name}: header error at {available}: {error:?}"),
        }
    }

    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 7)
        .collect();
    let mut component_rows = [0usize; 4];
    let mut suspensions = 0;

    loop {
        let rows: Vec<usize> = layout[..count]
            .iter()
            .enumerate()
            .map(|(index, plane)| {
                (plane.height - component_rows[index]).min(plane.vertical_sampling_factor * 8)
            })
            .collect();
        let required: Vec<usize> = layout[..count]
            .iter()
            .enumerate()
            .map(|(index, plane)| pull_required_len(rows[index], strides[index], plane.width))
            .collect();
        let mut storage: Vec<Vec<u8>> = required
            .iter()
            .map(|length| vec![SENTINEL; length + GUARD])
            .collect();
        let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();

        match raw.decode_next_imcu_row(&mut refs, &strides).unwrap() {
            RawImcuRowStatus::NeedMoreInput => {
                suspensions += 1;
                assert!(storage.iter().flatten().all(|byte| *byte == SENTINEL));
                assert!(available < data.len(), "{name}: suspended with full input");
                available = (available + step).min(data.len());
                limit.set(available);
            }
            RawImcuRowStatus::RowReady { rows_written } => {
                for index in 0..count {
                    assert_eq!(rows_written[index], rows[index]);
                    for row in 0..rows_written[index] {
                        let actual_start = row * strides[index];
                        let expected_start = (component_rows[index] + row) * layout[index].stride;
                        assert_eq!(
                            &storage[index][actual_start..actual_start + layout[index].width],
                            &whole[index][expected_start..expected_start + layout[index].width],
                            "{name}: component {index}, row {}",
                            component_rows[index] + row
                        );
                    }
                    assert!(storage[index][required[index]..]
                        .iter()
                        .all(|byte| *byte == SENTINEL));
                    component_rows[index] += rows_written[index];
                }
            }
            RawImcuRowStatus::Complete => break,
            _ => unreachable!("unknown raw iMCU-row status"),
        }
    }

    assert!(
        suspensions >= minimum_suspensions,
        "{name}: expected at least {minimum_suspensions} suspensions, got {suspensions}"
    );
    for index in 0..count {
        assert_eq!(component_rows[index], layout[index].height);
    }
}

#[test]
fn exact_minimum_lengths_cover_sampling_modes_and_final_stripes() {
    for bytes in [
        &include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg")[..],
        &include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg")[..],
        &include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg")[..],
        &include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg")[..],
        &include_bytes!("../../../test-images/jpeg/fox410.jpg")[..],
    ] {
        assert_exact_sized_pull_matches_whole(bytes);
    }

    let sampling_411 = with_luma_sampling(
        include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg"),
        0x41,
    );
    assert_exact_sized_pull_matches_whole(&sampling_411);
}

#[test]
fn suspension_cut_points_are_atomic_and_retry_exactly() {
    assert_incremental_pull_is_atomic(
        "baseline_bytewise",
        include_bytes!("../../../test-images/jpeg/app14/baseline_rgb_interleaved.jpg"),
        1,
        100,
    );
    assert_incremental_pull_is_atomic(
        "progressive_restart",
        include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg"),
        1,
        100,
    );
    assert_incremental_pull_is_atomic(
        "multi_sos",
        include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
        1,
        10,
    );
}

#[test]
#[cfg(feature = "arith")]
fn arithmetic_suspension_cut_points_are_atomic_and_retry_exactly() {
    assert_incremental_pull_is_atomic(
        "arithmetic_restart",
        include_bytes!("../../../test-images/jpeg/arith/seq-restart.jpg"),
        1,
        10,
    );
    assert_incremental_pull_is_atomic(
        "arithmetic_progressive",
        include_bytes!("../../../test-images/jpeg/arith/prog.jpg"),
        1,
        10,
    );
}

#[test]
fn exact_minimum_plane_lengths_succeed() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 13)
        .collect();
    let expected_rows: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.vertical_sampling_factor * 8)
        .collect();
    let mut storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .enumerate()
        .map(|(index, plane)| {
            vec![0xCD; pull_required_len(expected_rows[index], strides[index], plane.width)]
        })
        .collect();
    let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();

    let RawImcuRowStatus::RowReady { rows_written } =
        raw.decode_next_imcu_row(&mut refs, &strides).unwrap()
    else {
        panic!("first iMCU row was not ready")
    };
    assert_eq!(&rows_written[..count], expected_rows);
}

#[test]
fn one_byte_short_of_minimum_is_rejected() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");

    for short_index in 0..3 {
        let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
        decoder.decode_headers().unwrap();
        let mut raw = decoder.raw_output();
        let layout = raw.layout().unwrap();
        let count = raw.num_components().unwrap();
        let strides: Vec<usize> = layout[..count]
            .iter()
            .map(|plane| plane.width + 13)
            .collect();
        let rows: Vec<usize> = layout[..count]
            .iter()
            .map(|plane| plane.vertical_sampling_factor * 8)
            .collect();
        let required: Vec<usize> = layout[..count]
            .iter()
            .enumerate()
            .map(|(index, plane)| pull_required_len(rows[index], strides[index], plane.width))
            .collect();
        let mut storage: Vec<Vec<u8>> = required
            .iter()
            .enumerate()
            .map(|(index, length)| vec![0xCD; length - usize::from(index == short_index)])
            .collect();
        let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();

        assert!(matches!(
            raw.decode_next_imcu_row(&mut refs, &strides),
            Err(DecodeErrors::TooSmallOutput(needed, actual))
                if needed == required[short_index] && actual + 1 == needed
        ));
        assert!(storage.iter().flatten().all(|byte| *byte == 0xCD));
    }
}

#[test]
fn overflowing_minimum_length_is_rejected() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let mut strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    strides[0] = usize::MAX;
    let mut storage: Vec<Vec<u8>> = (0..count).map(|_| Vec::new()).collect();
    let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();

    assert!(matches!(
        raw.decode_next_imcu_row(&mut refs, &strides),
        Err(DecodeErrors::FormatStatic(
            "raw iMCU-row plane size overflow"
        ))
    ));
}

#[test]
fn one_byte_short_of_final_clipped_stripe_is_rejected() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 13)
        .collect();
    let mut component_rows = [0usize; 4];

    loop {
        let rows: Vec<usize> = layout[..count]
            .iter()
            .enumerate()
            .map(|(index, plane)| {
                (plane.height - component_rows[index]).min(plane.vertical_sampling_factor * 8)
            })
            .collect();
        let required: Vec<usize> = layout[..count]
            .iter()
            .enumerate()
            .map(|(index, plane)| pull_required_len(rows[index], strides[index], plane.width))
            .collect();
        let final_stripe = rows
            .iter()
            .enumerate()
            .all(|(index, rows)| component_rows[index] + rows == layout[index].height);
        let mut storage: Vec<Vec<u8>> = required
            .iter()
            .enumerate()
            .map(|(index, length)| vec![0xCD; length - usize::from(final_stripe && index == 0)])
            .collect();
        let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();

        if final_stripe {
            assert!(matches!(
                raw.decode_next_imcu_row(&mut refs, &strides),
                Err(DecodeErrors::TooSmallOutput(needed, actual))
                    if needed == required[0] && actual + 1 == needed
            ));
            assert!(storage.iter().flatten().all(|byte| *byte == 0xCD));
            break;
        }

        let RawImcuRowStatus::RowReady { rows_written } =
            raw.decode_next_imcu_row(&mut refs, &strides).unwrap()
        else {
            panic!("non-final stripe was not ready")
        };
        for index in 0..count {
            component_rows[index] += rows_written[index];
        }
    }
}

#[test]
fn recreated_session_continues_from_the_next_imcu_row() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");

    let mut expected_decoder = JpegDecoder::new(ZCursor::new(bytes));
    expected_decoder.decode_headers().unwrap();
    let mut expected_session = expected_decoder.raw_output();
    let layout = expected_session.layout().unwrap();
    let count = expected_session.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    let make_storage = || {
        layout[..count]
            .iter()
            .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
            .collect::<Vec<Vec<u8>>>()
    };
    let mut expected_first = make_storage();
    let mut expected_first_refs: Vec<&mut [u8]> =
        expected_first.iter_mut().map(Vec::as_mut_slice).collect();
    expected_session
        .decode_next_imcu_row(&mut expected_first_refs, &strides)
        .unwrap();
    let mut expected_second = make_storage();
    let mut expected_second_refs: Vec<&mut [u8]> =
        expected_second.iter_mut().map(Vec::as_mut_slice).collect();
    let expected_status = expected_session
        .decode_next_imcu_row(&mut expected_second_refs, &strides)
        .unwrap();

    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut first = make_storage();
    {
        let mut session = decoder.raw_output();
        let mut refs: Vec<&mut [u8]> = first.iter_mut().map(Vec::as_mut_slice).collect();
        session.decode_next_imcu_row(&mut refs, &strides).unwrap();
    }
    let mut second = make_storage();
    let actual_status = {
        let mut session = decoder.raw_output();
        let mut refs: Vec<&mut [u8]> = second.iter_mut().map(Vec::as_mut_slice).collect();
        session.decode_next_imcu_row(&mut refs, &strides).unwrap()
    };

    assert_eq!(first, expected_first);
    assert_eq!(actual_status, expected_status);
    assert_eq!(second, expected_second);
}

#[test]
fn completion_is_sticky_and_new_session_replays() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();

    let mut first_rows = None;
    loop {
        let mut storage: Vec<Vec<u8>> = layout[..count]
            .iter()
            .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
            .collect();
        let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();
        match raw.decode_next_imcu_row(&mut refs, &strides).unwrap() {
            RawImcuRowStatus::RowReady { rows_written } => {
                first_rows.get_or_insert((rows_written, storage));
            }
            RawImcuRowStatus::Complete => break,
            RawImcuRowStatus::NeedMoreInput => panic!("one-shot input suspended"),
            _ => unreachable!("unknown raw iMCU-row status")
        }
    }

    let mut empty: Vec<&mut [u8]> = Vec::new();
    assert_eq!(
        raw.decode_next_imcu_row(&mut empty, &[]).unwrap(),
        RawImcuRowStatus::Complete
    );
    let mut exhausted_storage: Vec<Vec<u8>> = (0..count).map(|_| Vec::new()).collect();
    let mut exhausted_planes: Vec<&mut [u8]> = exhausted_storage
        .iter_mut()
        .map(Vec::as_mut_slice)
        .collect();
    assert_eq!(
        raw.decode_next_imcu_row(&mut exhausted_planes, &strides)
            .unwrap(),
        RawImcuRowStatus::Complete
    );
    drop(raw);

    let mut replay = decoder.raw_output();
    let mut storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();
    let rows_written = match replay.decode_next_imcu_row(&mut refs, &strides).unwrap() {
        RawImcuRowStatus::RowReady { rows_written } => rows_written,
        _ => panic!("replay did not yield its first row")
    };
    let (expected_rows, expected_storage) = first_rows.unwrap();
    assert_eq!(rows_written, expected_rows);
    assert_eq!(storage, expected_storage);
}

#[test]
fn cancellation_does_not_publish_a_row() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    decoder.set_cancel(|| true);
    decoder.set_cancel_interval(1);
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    let mut storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0xCD; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();

    let error = raw.decode_next_imcu_row(&mut refs, &strides).unwrap_err();
    assert!(matches!(error, DecodeErrors::Cancelled));
    assert!(storage.iter().flatten().all(|byte| *byte == 0xCD));
}

#[test]
fn cancellation_during_a_stripe_is_atomic_and_retryable() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let (whole, layout, count) = decode_whole(bytes);
    let polls = Arc::new(AtomicUsize::new(0));
    let cancel_after = Arc::new(AtomicUsize::new(1));
    let check_polls = Arc::clone(&polls);
    let check_cancel_after = Arc::clone(&cancel_after);
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    decoder.set_cancel(move || {
        check_polls.fetch_add(1, Ordering::SeqCst) >= check_cancel_after.load(Ordering::SeqCst)
    });
    decoder.set_cancel_interval(1);
    let mut raw = decoder.raw_output();
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 9)
        .collect();
    let rows: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.vertical_sampling_factor * 8)
        .collect();
    let required: Vec<usize> = layout[..count]
        .iter()
        .enumerate()
        .map(|(index, plane)| pull_required_len(rows[index], strides[index], plane.width))
        .collect();
    let mut cancelled: Vec<Vec<u8>> = required.iter().map(|length| vec![0xCD; *length]).collect();
    let mut cancelled_refs: Vec<&mut [u8]> = cancelled.iter_mut().map(Vec::as_mut_slice).collect();

    assert!(matches!(
        raw.decode_next_imcu_row(&mut cancelled_refs, &strides),
        Err(DecodeErrors::Cancelled)
    ));
    assert!(polls.load(Ordering::SeqCst) > 1);
    assert!(cancelled.iter().flatten().all(|byte| *byte == 0xCD));

    cancel_after.store(usize::MAX, Ordering::SeqCst);
    let mut retry: Vec<Vec<u8>> = required.iter().map(|length| vec![0; *length]).collect();
    let mut retry_refs: Vec<&mut [u8]> = retry.iter_mut().map(Vec::as_mut_slice).collect();
    let RawImcuRowStatus::RowReady { rows_written } =
        raw.decode_next_imcu_row(&mut retry_refs, &strides).unwrap()
    else {
        panic!("retry did not produce the first stripe")
    };
    for index in 0..count {
        assert_eq!(rows_written[index], rows[index]);
        for row in 0..rows[index] {
            assert_eq!(
                &retry[index][row * strides[index]..row * strides[index] + layout[index].width],
                &whole[index]
                    [row * layout[index].stride..row * layout[index].stride + layout[index].width]
            );
        }
    }
}

#[test]
fn buffered_baseline_cancellation_between_rows_is_atomic() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let (whole, _, _) = decode_whole(bytes);
    let cancelled = Arc::new(AtomicBool::new(false));
    let check = Arc::clone(&cancelled);
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.set_cancel(move || check.load(Ordering::SeqCst));
    decoder.set_cancel_interval(1);
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();

    let mut first: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut first_refs: Vec<&mut [u8]> = first.iter_mut().map(Vec::as_mut_slice).collect();
    let first_rows = match raw.decode_next_imcu_row(&mut first_refs, &strides).unwrap() {
        RawImcuRowStatus::RowReady { rows_written } => rows_written,
        _ => panic!("first pull did not yield a row")
    };

    cancelled.store(true, Ordering::SeqCst);
    let mut cancelled_storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0xCD; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut cancelled_refs: Vec<&mut [u8]> = cancelled_storage
        .iter_mut()
        .map(Vec::as_mut_slice)
        .collect();
    assert!(matches!(
        raw.decode_next_imcu_row(&mut cancelled_refs, &strides),
        Err(DecodeErrors::Cancelled)
    ));
    assert!(cancelled_storage.iter().flatten().all(|byte| *byte == 0xCD));

    cancelled.store(false, Ordering::SeqCst);
    let mut retry: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut retry_refs: Vec<&mut [u8]> = retry.iter_mut().map(Vec::as_mut_slice).collect();
    let retry_rows = match raw.decode_next_imcu_row(&mut retry_refs, &strides).unwrap() {
        RawImcuRowStatus::RowReady { rows_written } => rows_written,
        _ => panic!("retry did not yield the next row")
    };
    for index in 0..count {
        for row in 0..retry_rows[index] {
            let retry_start = row * strides[index];
            let whole_start = (first_rows[index] + row) * layout[index].stride;
            assert_eq!(
                &retry[index][retry_start..retry_start + layout[index].width],
                &whole[index][whole_start..whole_start + layout[index].width],
                "component {index}, retried row {row}"
            );
        }
    }
}

#[test]
fn switching_to_pixel_decode_replays_from_scan_start() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = JpegDecoder::new(ZCursor::new(bytes)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    let mut storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();
    assert!(matches!(
        raw.decode_next_imcu_row(&mut refs, &strides).unwrap(),
        RawImcuRowStatus::RowReady { .. }
    ));
    drop(raw);

    let mut actual = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut actual).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn switching_pull_to_whole_raw_is_rejected() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    let mut stripe: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut stripe_refs: Vec<&mut [u8]> = stripe.iter_mut().map(Vec::as_mut_slice).collect();
    assert!(matches!(
        raw.decode_next_imcu_row(&mut stripe_refs, &strides)
            .unwrap(),
        RawImcuRowStatus::RowReady { .. }
    ));

    let mut whole: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.byte_size])
        .collect();
    let mut whole_refs: Vec<&mut [u8]> = whole.iter_mut().map(Vec::as_mut_slice).collect();
    assert!(matches!(
        raw.decode_into(&mut whole_refs),
        Err(DecodeErrors::FormatStatic(_))
    ));
}

#[test]
fn invalid_pull_does_not_claim_raw_output_ownership() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();

    assert!(matches!(
        raw.decode_next_imcu_row(&mut [], &[]),
        Err(DecodeErrors::Format(_))
    ));

    let mut whole: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.byte_size])
        .collect();
    let mut whole_refs: Vec<&mut [u8]> = whole.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into(&mut whole_refs).unwrap();
    assert!(whole
        .iter()
        .all(|plane| plane.iter().any(|byte| *byte != 0)));
}

#[test]
fn invalid_pull_stride_does_not_claim_raw_output_ownership() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let mut strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    strides[0] -= 1;
    let mut stripe: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut stripe_refs: Vec<&mut [u8]> = stripe.iter_mut().map(Vec::as_mut_slice).collect();

    assert!(matches!(
        raw.decode_next_imcu_row(&mut stripe_refs, &strides),
        Err(DecodeErrors::Format(_))
    ));

    let mut whole: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.byte_size])
        .collect();
    let mut whole_refs: Vec<&mut [u8]> = whole.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into(&mut whole_refs).unwrap();
    assert!(whole
        .iter()
        .all(|plane| plane.iter().any(|byte| *byte != 0)));
}

#[test]
fn changing_options_aborts_pull_and_replays_from_scan_start() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count].iter().map(|plane| plane.width).collect();
    let mut first: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut first_refs: Vec<&mut [u8]> = first.iter_mut().map(Vec::as_mut_slice).collect();
    assert!(matches!(
        raw.decode_next_imcu_row(&mut first_refs, &strides).unwrap(),
        RawImcuRowStatus::RowReady { .. }
    ));
    drop(raw);

    decoder.set_options(DecoderOptions::default());
    let mut replay = decoder.raw_output();
    let mut replay_first: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut replay_refs: Vec<&mut [u8]> = replay_first.iter_mut().map(Vec::as_mut_slice).collect();
    assert!(matches!(
        replay
            .decode_next_imcu_row(&mut replay_refs, &strides)
            .unwrap(),
        RawImcuRowStatus::RowReady { .. }
    ));
    assert_eq!(replay_first, first);
}

#[test]
fn dnl_pull_is_rejected_before_false_completion() {
    let bytes = include_bytes!("images/dnl_image.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let initial_layout = raw.layout().unwrap();
    assert_eq!(initial_layout[0].height, 0);
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = initial_layout[..count]
        .iter()
        .map(|plane| plane.width)
        .collect();
    let mut storage: Vec<Vec<u8>> = initial_layout[..count]
        .iter()
        .map(|plane| vec![0; plane.width * plane.vertical_sampling_factor * 8])
        .collect();
    let mut refs: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();
    assert!(matches!(
        raw.decode_next_imcu_row(&mut refs, &strides),
        Err(DecodeErrors::FormatStatic(
            "raw output does not support DNL images"
        ))
    ));
}
