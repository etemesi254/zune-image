use std::cell::Cell;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use zune_core::bytestream::{ZByteReaderTrait, ZCursor};
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{JpegDecoder, ScanlineReadStatus, ScanlineStatus};

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

fn assert_incremental_scanlines_match_decode(name: &str, data: &[u8], step: usize) {
    let expected = JpegDecoder::new(ZCursor::new(data)).decode().unwrap();
    let limit = Rc::new(Cell::new(0));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    let mut available = 0;
    let mut scanlines = decoder.scanline_output();
    loop {
        available = (available + step).min(data.len());
        limit.set(available);
        match scanlines.start().unwrap() {
            ScanlineStatus::Ready => break,
            ScanlineStatus::NeedMoreInput => continue,
            ScanlineStatus::Complete => panic!("{name}: completed while starting"),
            _ => panic!("{name}: unknown start status")
        }
    }

    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    let mut actual = vec![0; row_bytes * height];
    let mut saw_suspension = false;
    while scanlines.output_scanline() < height {
        let row = scanlines.output_scanline();
        let mut storage = vec![0xCD; row_bytes];
        match scanlines.read_scanlines(&mut storage, row_bytes).unwrap() {
            ScanlineReadStatus::RowsProcessed { rows: 1 } => {
                actual[row * row_bytes..(row + 1) * row_bytes].copy_from_slice(&storage);
            }
            ScanlineReadStatus::NeedMoreInput => {
                saw_suspension = true;
                assert!(storage.iter().all(|byte| *byte == 0xCD));
                assert_eq!(scanlines.output_scanline(), row);
                if matches!(name, "progressive" | "multi_sos") {
                    assert_eq!(
                        row, 0,
                        "{name}: buffered output exposed provisional preview rows"
                    );
                }
                assert!(available < data.len(), "{name}: suspended with full input");
                available = (available + step).min(data.len());
                limit.set(available);
            }
            status => panic!("{name}: unexpected scanline status {status:?}")
        }
    }
    assert!(
        saw_suspension,
        "{name}: test never suspended during row output"
    );
    assert_eq!(actual, expected, "{name}: incremental output mismatch");

    loop {
        match scanlines.finish().unwrap() {
            ScanlineStatus::Complete => break,
            ScanlineStatus::NeedMoreInput => {
                assert!(
                    available < data.len(),
                    "{name}: finish suspended with full input"
                );
                available = (available + step).min(data.len());
                limit.set(available);
            }
            ScanlineStatus::Ready => panic!("{name}: finish returned Ready"),
            _ => panic!("{name}: unknown finish status")
        }
    }
}

fn assert_scanlines_match_decode(bytes: &[u8], colorspace: ColorSpace, rows_per_read: usize) {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    let expected = JpegDecoder::new_with_options(ZCursor::new(bytes), options)
        .decode()
        .unwrap();
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    let stride = row_bytes + 5;
    let mut actual = vec![0; row_bytes * height];

    while scanlines.output_scanline() < height {
        let row_start = scanlines.output_scanline();
        let requested_rows = rows_per_read.min(height - row_start);
        let mut storage = vec![0xCD; stride * requested_rows];
        let status = scanlines.read_scanlines(&mut storage, stride).unwrap();
        let ScanlineReadStatus::RowsProcessed { rows } = status else {
            panic!(
                "one-shot scanline input returned {status:?} at row {}",
                scanlines.output_scanline()
            )
        };
        assert!(rows > 0 && rows <= requested_rows);
        for row in 0..rows {
            actual[(row_start + row) * row_bytes..(row_start + row + 1) * row_bytes]
                .copy_from_slice(&storage[row * stride..row * stride + row_bytes]);
            assert!(storage[row * stride + row_bytes..(row + 1) * stride]
                .iter()
                .all(|byte| *byte == 0xCD));
        }
    }
    assert_eq!(actual, expected);
}

fn decode_all_scanlines<T: ZByteReaderTrait>(decoder: &mut JpegDecoder<T>) -> Vec<u8> {
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    let mut output = vec![0; row_bytes * height];
    while scanlines.output_scanline() < height {
        let row = scanlines.output_scanline();
        assert!(matches!(
            scanlines
                .read_scanlines(&mut output[row * row_bytes..], row_bytes)
                .unwrap(),
            ScanlineReadStatus::RowsProcessed { rows } if rows > 0
        ));
    }
    assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
    output
}

#[test]
fn baseline_420_one_row_reads_match_decode_with_custom_stride() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = JpegDecoder::new(ZCursor::new(bytes)).decode().unwrap();

    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    let stride = row_bytes + 7;
    let mut actual = vec![0; row_bytes * height];

    let mut empty = [];
    assert_eq!(
        scanlines.read_scanlines(&mut empty, 0).unwrap(),
        ScanlineReadStatus::RowsProcessed { rows: 0 }
    );
    assert_eq!(scanlines.output_scanline(), 0);

    for row in 0..height {
        let mut storage = vec![0xCD; stride];
        assert_eq!(
            scanlines.read_scanlines(&mut storage, stride).unwrap(),
            ScanlineReadStatus::RowsProcessed { rows: 1 }
        );
        actual[row * row_bytes..(row + 1) * row_bytes].copy_from_slice(&storage[..row_bytes]);
        assert!(storage[row_bytes..].iter().all(|byte| *byte == 0xCD));
        assert_eq!(scanlines.output_scanline(), row + 1);
    }

    assert_eq!(actual, expected);
    let mut storage = vec![0xCD; stride];
    assert_eq!(
        scanlines.read_scanlines(&mut storage, stride).unwrap(),
        ScanlineReadStatus::Complete
    );
    assert!(storage.iter().all(|byte| *byte == 0xCD));
    assert_eq!(
        scanlines.read_scanlines(&mut storage, stride).unwrap(),
        ScanlineReadStatus::Complete
    );
}

#[test]
fn baseline_scanlines_match_supported_output_colorspaces() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    for colorspace in [
        ColorSpace::RGB,
        ColorSpace::RGBA,
        ColorSpace::Luma,
        ColorSpace::BGR,
        ColorSpace::BGRA
    ] {
        assert_scanlines_match_decode(bytes, colorspace, 7);
    }
}

#[test]
fn batched_baseline_scanlines_match_decode() {
    assert_scanlines_match_decode(
        include_bytes!("../../../test-images/jpeg/2029.jpg"),
        ColorSpace::RGB,
        64
    );
}

#[test]
fn odd_dimension_scanlines_match_decode() {
    assert_scanlines_match_decode(
        include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg"),
        ColorSpace::RGB,
        3
    );
}

#[test]
fn oversized_final_read_does_not_write_past_logical_rows() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    while scanlines.output_scanline() < height - 1 {
        assert!(matches!(
            scanlines.skip_scanlines(height - 1 - scanlines.output_scanline()).unwrap(),
            ScanlineReadStatus::RowsProcessed { rows } if rows > 0
        ));
    }

    let capacity = 80;
    let mut storage = vec![0xCD; row_bytes * capacity];
    assert_eq!(
        scanlines.read_scanlines(&mut storage, row_bytes).unwrap(),
        ScanlineReadStatus::RowsProcessed { rows: 1 }
    );
    assert!(storage[row_bytes..].iter().all(|byte| *byte == 0xCD));
}

#[test]
fn progressive_scanlines_match_decode() {
    assert_scanlines_match_decode(
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
        ColorSpace::RGB,
        5
    );
}

#[test]
fn multi_sos_scanlines_match_decode() {
    assert_scanlines_match_decode(
        include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
        ColorSpace::RGB,
        5
    );
}

#[test]
fn skip_scanlines_preserves_following_rows_and_end_behavior() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = JpegDecoder::new(ZCursor::new(bytes)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    let skip = 19;

    assert_eq!(
        scanlines.skip_scanlines(0).unwrap(),
        ScanlineReadStatus::RowsProcessed { rows: 0 }
    );
    let mut skipped = 0;
    while skipped < skip {
        let ScanlineReadStatus::RowsProcessed { rows } =
            scanlines.skip_scanlines(skip - skipped).unwrap()
        else {
            panic!("skip did not make progress")
        };
        skipped += rows;
    }
    let mut row = vec![0; row_bytes];
    assert_eq!(
        scanlines.read_scanlines(&mut row, row_bytes).unwrap(),
        ScanlineReadStatus::RowsProcessed { rows: 1 }
    );
    assert_eq!(&row, &expected[skip * row_bytes..(skip + 1) * row_bytes]);

    let mut tail_skipped = 0;
    while scanlines.output_scanline() < height {
        let ScanlineReadStatus::RowsProcessed { rows } =
            scanlines.skip_scanlines(usize::MAX).unwrap()
        else {
            panic!("tail skip did not make progress")
        };
        tail_skipped += rows;
    }
    assert_eq!(tail_skipped, height - skip - 1);
    assert_eq!(
        scanlines.skip_scanlines(1).unwrap(),
        ScanlineReadStatus::Complete
    );
    assert_eq!(scanlines.output_scanline(), height);
    assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
    assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
}

#[test]
fn finish_discards_unread_rows_and_completes() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
    assert_eq!(
        scanlines.output_scanline(),
        scanlines.output_height().unwrap()
    );
}

#[test]
fn read_past_end_preserves_unfinished_sequence_until_finish() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    let mut row = vec![0; row_bytes];
    while scanlines.output_scanline() < height {
        assert_eq!(
            scanlines.read_scanlines(&mut row, row_bytes).unwrap(),
            ScanlineReadStatus::RowsProcessed { rows: 1 }
        );
    }
    assert_eq!(
        scanlines.read_scanlines(&mut row, row_bytes).unwrap(),
        ScanlineReadStatus::Complete
    );
    drop(scanlines);

    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    assert_eq!(scanlines.output_scanline(), height);
    assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
}

#[test]
fn short_scanline_buffers_are_rejected_without_progress() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();

    let mut row = vec![0xCD; row_bytes];
    assert!(matches!(
        scanlines.read_scanlines(&mut row, row_bytes - 1),
        Err(DecodeErrors::Format(_))
    ));
    let mut short = vec![0xCD; row_bytes - 1];
    assert!(matches!(
        scanlines.read_scanlines(&mut short, row_bytes),
        Err(DecodeErrors::TooSmallOutput(_, _))
    ));
    assert_eq!(scanlines.output_scanline(), 0);
    assert!(row.iter().chain(short.iter()).all(|byte| *byte == 0xCD));
}

#[test]
fn cancellation_does_not_publish_scanlines_and_can_retry() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = JpegDecoder::new(ZCursor::new(bytes)).decode().unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let check = Arc::clone(&cancelled);
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.set_cancel(move || check.load(Ordering::SeqCst));
    decoder.set_cancel_interval(1);
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let mut row = vec![0xCD; row_bytes];
    cancelled.store(true, Ordering::SeqCst);

    assert!(matches!(
        scanlines.read_scanlines(&mut row, row_bytes),
        Err(DecodeErrors::Cancelled)
    ));
    assert!(row.iter().all(|byte| *byte == 0xCD));
    assert_eq!(scanlines.output_scanline(), 0);

    cancelled.store(false, Ordering::SeqCst);
    assert_eq!(
        scanlines.read_scanlines(&mut row, row_bytes).unwrap(),
        ScanlineReadStatus::RowsProcessed { rows: 1 }
    );
    assert_eq!(&row, &expected[..row_bytes]);
}

#[test]
fn cancellation_after_a_committed_batch_is_reported_on_the_next_read() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = JpegDecoder::new(ZCursor::new(bytes)).decode().unwrap();
    let polls = Arc::new(AtomicUsize::new(0));
    let cancel_at = Arc::new(AtomicUsize::new(usize::MAX));
    let check_polls = Arc::clone(&polls);
    let check_cancel_at = Arc::clone(&cancel_at);
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.set_cancel(move || {
        check_polls.fetch_add(1, Ordering::SeqCst) >= check_cancel_at.load(Ordering::SeqCst)
    });
    decoder.set_cancel_interval(1);
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let mut output = vec![0xCD; row_bytes * 100];
    polls.store(0, Ordering::SeqCst);
    cancel_at.store(2, Ordering::SeqCst);

    let ScanlineReadStatus::RowsProcessed { rows: first_rows } =
        scanlines.read_scanlines(&mut output, row_bytes).unwrap()
    else {
        panic!("first scanline batch did not complete")
    };
    assert!(first_rows > 0);
    assert_eq!(scanlines.output_scanline(), first_rows);
    assert_eq!(
        &output[..first_rows * row_bytes],
        &expected[..first_rows * row_bytes]
    );

    output.fill(0xCD);
    assert!(matches!(
        scanlines.read_scanlines(&mut output, row_bytes),
        Err(DecodeErrors::Cancelled)
    ));
    assert!(output.iter().all(|byte| *byte == 0xCD));
    assert_eq!(scanlines.output_scanline(), first_rows);

    cancel_at.store(usize::MAX, Ordering::SeqCst);
    let ScanlineReadStatus::RowsProcessed { rows } =
        scanlines.read_scanlines(&mut output, row_bytes).unwrap()
    else {
        panic!("scanline retry did not complete")
    };
    assert_eq!(
        &output[..rows * row_bytes],
        &expected[first_rows * row_bytes..(first_rows + rows) * row_bytes]
    );
}

#[test]
fn buffered_cancellation_between_scanline_batches_is_atomic() {
    let bytes = include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg");
    let expected = JpegDecoder::new(ZCursor::new(bytes)).decode().unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let check = Arc::clone(&cancelled);
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.set_cancel(move || check.load(Ordering::SeqCst));
    decoder.set_cancel_interval(1);
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let mut first = vec![0; row_bytes * 32];
    let ScanlineReadStatus::RowsProcessed { rows: first_rows } =
        scanlines.read_scanlines(&mut first, row_bytes).unwrap()
    else {
        panic!("first buffered batch did not produce rows")
    };
    assert!(first_rows > 0);

    cancelled.store(true, Ordering::SeqCst);
    let mut cancelled_rows = vec![0xCD; row_bytes * 32];
    assert!(matches!(
        scanlines.read_scanlines(&mut cancelled_rows, row_bytes),
        Err(DecodeErrors::Cancelled)
    ));
    assert!(cancelled_rows.iter().all(|byte| *byte == 0xCD));
    assert_eq!(scanlines.output_scanline(), first_rows);

    cancelled.store(false, Ordering::SeqCst);
    let ScanlineReadStatus::RowsProcessed { rows } = scanlines
        .read_scanlines(&mut cancelled_rows, row_bytes)
        .unwrap()
    else {
        panic!("buffered cancellation retry did not produce rows")
    };
    assert_eq!(
        &cancelled_rows[..rows * row_bytes],
        &expected[first_rows * row_bytes..(first_rows + rows) * row_bytes]
    );
}

#[derive(Clone, Copy)]
enum OptionChangePrelude {
    Headers,
    PixelCheckpoint,
    Scanline
}

#[test]
fn changing_output_options_rebuilds_derived_state_for_replay() {
    let baseline = include_bytes!("../../../test-images/jpeg/2029.jpg").as_slice();
    let progressive =
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg").as_slice();
    for (name, bytes, initial, output, prelude) in [
        (
            "pixel checkpoint",
            baseline,
            ColorSpace::Luma,
            ColorSpace::RGB,
            OptionChangePrelude::PixelCheckpoint
        ),
        (
            "baseline coeff",
            baseline,
            ColorSpace::Luma,
            ColorSpace::RGB,
            OptionChangePrelude::Scanline
        ),
        (
            "progressive coeff",
            progressive,
            ColorSpace::Luma,
            ColorSpace::RGB,
            OptionChangePrelude::Scanline
        ),
        (
            "RGB converter",
            baseline,
            ColorSpace::BGRA,
            ColorSpace::RGB,
            OptionChangePrelude::Headers
        ),
        (
            "BGR converter",
            baseline,
            ColorSpace::RGB,
            ColorSpace::BGR,
            OptionChangePrelude::Headers
        ),
        (
            "RGBA converter",
            baseline,
            ColorSpace::RGB,
            ColorSpace::RGBA,
            OptionChangePrelude::Headers
        ),
        (
            "BGRA converter",
            baseline,
            ColorSpace::RGB,
            ColorSpace::BGRA,
            OptionChangePrelude::Headers
        )
    ] {
        let initial_options = DecoderOptions::default().jpeg_set_out_colorspace(initial);
        let output_options = DecoderOptions::default().jpeg_set_out_colorspace(output);
        let expected = JpegDecoder::new_with_options(ZCursor::new(bytes), output_options)
            .decode()
            .unwrap();

        let initial_limit = match prelude {
            OptionChangePrelude::PixelCheckpoint => bytes.len() * 60 / 100,
            OptionChangePrelude::Headers | OptionChangePrelude::Scanline => bytes.len()
        };
        let limit = Rc::new(Cell::new(initial_limit));
        let cursor = GrowableCursor::new(bytes, Rc::clone(&limit));
        let mut decoder = JpegDecoder::new_with_options(cursor, initial_options);
        match prelude {
            OptionChangePrelude::Headers => decoder.decode_headers().unwrap(),
            OptionChangePrelude::Scanline => {
                let mut scanlines = decoder.scanline_output();
                assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
                let row_bytes = scanlines.output_row_bytes().unwrap();
                let mut row = vec![0; row_bytes];
                assert_eq!(
                    scanlines.read_scanlines(&mut row, row_bytes).unwrap(),
                    ScanlineReadStatus::RowsProcessed { rows: 1 }
                );
            }
            OptionChangePrelude::PixelCheckpoint => {
                decoder.set_incremental_mode(true);
                decoder.decode_headers().unwrap();
                let mut old_output = vec![0; decoder.output_buffer_size().unwrap()];
                for _ in 0..2 {
                    let error = decoder.decode_into(&mut old_output).unwrap_err();
                    assert!(error.is_recoverable_eof());
                }
                assert!(decoder.decoded_scanlines().unwrap_or(0) > 0);
                limit.set(bytes.len());
            }
        }
        let committed_scans = decoder.decoded_scans();
        decoder.set_options(output_options);
        assert_eq!(decoder.decoded_output_bytes(), Some(0), "{name}");
        assert_eq!(decoder.decoded_scanlines(), Some(0), "{name}");
        if let Some(committed_scans) = committed_scans {
            assert!(committed_scans > 0, "{name}");
            assert_eq!(decoder.decoded_scans(), Some(committed_scans), "{name}");
            assert_eq!(decoder.decoded_preview_output_bytes(), Some(0), "{name}");
            assert_eq!(decoder.decoded_preview_scanlines(), Some(0), "{name}");
        }

        let actual = decode_all_scanlines(&mut decoder);
        assert_eq!(actual, expected, "{name}: {initial:?} -> {output:?}");
    }
}

#[test]
fn completed_scanline_sequence_can_replay() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let expected = JpegDecoder::new(ZCursor::new(bytes)).decode().unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
    drop(scanlines);

    let mut replay = decoder.scanline_output();
    assert_eq!(replay.start().unwrap(), ScanlineStatus::Ready);
    assert_eq!(replay.output_scanline(), 0);
    let row_bytes = replay.output_row_bytes().unwrap();
    let mut row = vec![0; row_bytes];
    assert_eq!(
        replay.read_scanlines(&mut row, row_bytes).unwrap(),
        ScanlineReadStatus::RowsProcessed { rows: 1 }
    );
    assert_eq!(&row, &expected[..row_bytes]);
}

#[test]
fn incremental_scanline_parity() {
    assert_incremental_scanlines_match_decode(
        "baseline",
        include_bytes!("../../../test-images/jpeg/synthetic_image.jpg"),
        37
    );
    assert_incremental_scanlines_match_decode(
        "restart",
        include_bytes!("../../../test-images/jpeg/four_components.jpg"),
        97
    );
    assert_incremental_scanlines_match_decode(
        "progressive",
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
        31
    );
    assert_incremental_scanlines_match_decode(
        "multi_sos",
        include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
        31
    );
}

#[test]
#[cfg(feature = "arith")]
fn arithmetic_incremental_scanline_parity() {
    assert_incremental_scanlines_match_decode(
        "arithmetic_restart",
        include_bytes!("../../../test-images/jpeg/arith/seq-restart.jpg"),
        23
    );
    assert_incremental_scanlines_match_decode(
        "arithmetic_progressive",
        include_bytes!("../../../test-images/jpeg/arith/prog.jpg"),
        23
    );
}

#[test]
fn dnl_scanlines_are_rejected_before_false_completion() {
    let bytes = include_bytes!("images/dnl_image.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let mut scanlines = decoder.scanline_output();
    assert!(matches!(
        scanlines.start(),
        Err(DecodeErrors::FormatStatic(
            "converted scanline output does not support DNL images"
        ))
    ));
    assert_eq!(scanlines.output_scanline(), 0);
}
