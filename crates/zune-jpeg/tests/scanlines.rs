/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for the libjpeg-turbo-style scanline output API
//! (`start_decompress` / `read_scanlines` / `finish_decompress`).
//!
//! Every test decodes the same image twice — once via the one-shot
//! `decode_into` and once via the scanline API — and checks the byte
//! streams match. That covers the various subsampling, output-colorspace,
//! progressive, and restart-interval permutations without us having to
//! hand-bake reference pixels.

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

/// Decode `data` via the scanline API with the given options, pulling
/// `chunk` scanlines per call. Returns the assembled buffer and the
/// number of scanlines actually written (which should equal output_height).
fn decode_via_scanlines(data: &[u8], options: DecoderOptions, chunk: usize) -> (Vec<u8>, usize) {
    let mut dec = JpegDecoder::new_with_options(ZCursor::new(data), options);
    dec.decode_headers().unwrap();
    dec.start_decompress().unwrap();

    let row_stride = dec.output_row_stride().unwrap();
    let height = dec.output_height().unwrap();

    let mut out = vec![0_u8; row_stride * height];
    let mut total = 0;
    while dec.next_scanline() < height {
        let buf = &mut out[total * row_stride..];
        let want = chunk.min(height - total);
        let got = dec.read_scanlines(buf, want).unwrap();
        if got == 0 {
            break;
        }
        total += got;
    }
    dec.finish_decompress().unwrap();
    (out, total)
}

fn decode_one_shot(data: &[u8], options: DecoderOptions) -> Vec<u8> {
    let mut dec = JpegDecoder::new_with_options(ZCursor::new(data), options);
    dec.decode().unwrap()
}

fn assert_matches_one_shot(data: &[u8], options: DecoderOptions, chunk: usize) {
    let baseline = decode_one_shot(data, options);
    let (scanlines, total) = decode_via_scanlines(data, options, chunk);
    let height = {
        let mut dec = JpegDecoder::new_with_options(ZCursor::new(data), options);
        dec.decode_headers().unwrap();
        dec.output_height().unwrap()
    };
    assert_eq!(total, height, "scanline count mismatch (chunk={chunk})");
    assert_eq!(
        scanlines.len(),
        baseline.len(),
        "buffer size mismatch (chunk={chunk})"
    );
    if scanlines != baseline {
        let mismatched = scanlines
            .iter()
            .zip(baseline.iter())
            .position(|(a, b)| a != b);
        panic!(
            "scanline output diverges from one-shot decode at byte {:?} (chunk={chunk})",
            mismatched
        );
    }
}

#[test]
fn scanlines_baseline_420_default_rgb() {
    let data = include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg");
    let opts = DecoderOptions::default();
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 8);
    assert_matches_one_shot(data, opts, 16);
    assert_matches_one_shot(data, opts, usize::MAX / 2);
}

#[test]
fn scanlines_baseline_444() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let opts = DecoderOptions::default();
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 7);
    assert_matches_one_shot(data, opts, 64);
}

#[test]
fn scanlines_baseline_422() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg");
    let opts = DecoderOptions::default();
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 16);
}

#[test]
fn scanlines_baseline_horiz_sampled() {
    let data = include_bytes!("../../../test-images/jpeg/medium_horiz_samp_2500x1786.jpg");
    let opts = DecoderOptions::default();
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 17);
}

#[test]
fn scanlines_baseline_vertical_sampled() {
    // Vertical sampling exercises the post_process path that holds over
    // the last row of an MCU for the next call's upsample.
    let data = include_bytes!("../../../test-images/jpeg/medium_vertical_samp_2500x1786.jpg");
    let opts = DecoderOptions::default();
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 16);
    assert_matches_one_shot(data, opts, 64);
}

#[test]
fn scanlines_baseline_rgba_output() {
    let data = include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg");
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 64);
}

#[test]
fn scanlines_baseline_luma_output_from_color() {
    // YCbCr -> Luma exercises the coeff=2 path.
    let data = include_bytes!("../../../test-images/jpeg/medium_vertical_samp_2500x1786.jpg");
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 32);
}

#[test]
fn scanlines_progressive_420() {
    let data = include_bytes!("../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg");
    let opts = DecoderOptions::default();
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 8);
    assert_matches_one_shot(data, opts, 100);
}

#[test]
fn scanlines_progressive_grayscale() {
    let data = include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg");
    let opts = DecoderOptions::default();
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 8);
}

#[test]
fn scanlines_multi_sos_baseline() {
    // sos_news.jpeg interleaves SOS markers with MCU data — historically a
    // tricky case for streaming. Pin its byte-for-byte output here so it
    // stays covered when later PRs swap in per-MCU-row decoding.
    let data = include_bytes!("../../../test-images/jpeg/sos_news.jpeg");
    let opts = DecoderOptions::default();
    assert_matches_one_shot(data, opts, 1);
    assert_matches_one_shot(data, opts, 32);
}

#[test]
fn scanlines_zero_lines_is_noop() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(data));
    dec.decode_headers().unwrap();
    dec.start_decompress().unwrap();
    let n = dec.read_scanlines(&mut [], 0).unwrap();
    assert_eq!(n, 0);
    assert_eq!(dec.next_scanline(), 0);
}

#[test]
fn scanlines_short_buffer_errors() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(data));
    dec.decode_headers().unwrap();
    dec.start_decompress().unwrap();
    let row_stride = dec.output_row_stride().unwrap();
    // Ask for 4 lines but only supply room for 2.
    let mut buf = vec![0_u8; row_stride * 2];
    let err = dec.read_scanlines(&mut buf, 4);
    assert!(err.is_err(), "short buffer should produce an error");
}

#[test]
fn scanlines_read_past_end_returns_zero() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(data));
    dec.decode_headers().unwrap();
    dec.start_decompress().unwrap();
    let row_stride = dec.output_row_stride().unwrap();
    let height = dec.output_height().unwrap();

    // Drain the whole image.
    let mut out = vec![0_u8; row_stride * height];
    let mut total = 0;
    while dec.next_scanline() < height {
        let n = dec.read_scanlines(&mut out[total * row_stride..], 1).unwrap();
        if n == 0 {
            break;
        }
        total += n;
    }
    assert_eq!(total, height);

    // Subsequent reads should return 0, never error.
    let mut tmp = vec![0_u8; row_stride];
    assert_eq!(dec.read_scanlines(&mut tmp, 1).unwrap(), 0);
}

#[test]
fn scanlines_double_start_errors() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(data));
    dec.decode_headers().unwrap();
    dec.start_decompress().unwrap();
    assert!(
        dec.start_decompress().is_err(),
        "second start_decompress without finish must error"
    );
    dec.finish_decompress().unwrap();
}

#[test]
fn scanlines_finish_decompress_idempotent() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(data));
    dec.decode_headers().unwrap();
    dec.start_decompress().unwrap();
    dec.finish_decompress().unwrap();
    // A second finish without an intervening start should also be a no-op.
    dec.finish_decompress().unwrap();
}

#[test]
fn scanlines_read_before_start_errors() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(data));
    dec.decode_headers().unwrap();
    let mut buf = vec![0_u8; 64 * 3];
    // No start_decompress yet — every form of read_scanlines must error,
    // including the num_lines == 0 case (precondition is checked first).
    assert!(dec.read_scanlines(&mut buf, 1).is_err());
    assert!(dec.read_scanlines(&mut buf, 0).is_err());
    assert!(dec.read_scanlines(&mut [], 0).is_err());
}

#[test]
fn scanlines_oversize_num_lines_ok() {
    // libjpeg-turbo documents that an oversize max_lines is not an error;
    // the call should just cap to whatever is left. Verify that here with
    // a buffer sized only for the actual remaining rows.
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let mut dec = JpegDecoder::new(ZCursor::new(data));
    dec.decode_headers().unwrap();
    dec.start_decompress().unwrap();
    let row_stride = dec.output_row_stride().unwrap();
    let height = dec.output_height().unwrap();

    // Buffer is exactly height rows — passing usize::MAX as num_lines
    // must succeed and write `height` rows.
    let mut buf = vec![0_u8; row_stride * height];
    let n = dec.read_scanlines(&mut buf, usize::MAX).unwrap();
    assert_eq!(n, height);
    // Past the end, oversize num_lines stays a no-op rather than erroring.
    let mut tmp = [0_u8; 0];
    assert_eq!(dec.read_scanlines(&mut tmp, usize::MAX).unwrap(), 0);
}

