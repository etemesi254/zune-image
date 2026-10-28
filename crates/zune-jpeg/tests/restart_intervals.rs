/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::{JpegDecoder, ScanlineReadStatus, ScanlineStatus};

// Issue #509's synthetic grayscale frame: 16 MCUs per row, DRI=10.
// Quantization 16 and DC +/-40 give exact native samples 48 and 208.
const IMAGE: &[u8] = include_bytes!("../../../test-images/jpeg/restart_cross_row_128x128.jpg");
const WIDTH: usize = 128;
const HEIGHT: usize = 128;
const SENTINEL: u8 = 0xa5;

fn assert_native_pixels(output: &[u8], channels: usize, stride: usize) {
    let row_bytes = WIDTH * channels;
    assert_eq!(output.len(), HEIGHT * stride);
    for (row_index, row) in output.chunks_exact(stride).enumerate() {
        for (column, pixel) in row[..row_bytes].chunks_exact(channels).enumerate() {
            let expected = if (48..80).contains(&column) { 208 } else { 48 };
            assert!(
                pixel.iter().all(|&sample| sample == expected),
                "pixel ({column},{row_index}): {pixel:?}, expected {expected}"
            );
        }
        assert!(row[row_bytes..].iter().all(|&byte| byte == SENTINEL));
    }
}

#[test]
fn restart_read_ahead_preserves_pending_mcus() {
    assert_eq!(IMAGE.len(), 326);
    for strict in [true, false] {
        for simd in [false, true] {
            for format in [ColorSpace::Luma, ColorSpace::RGB] {
                let options = DecoderOptions::default()
                    .set_strict_mode(strict)
                    .set_use_unsafe(simd)
                    .jpeg_set_out_colorspace(format);
                let mut decoder = JpegDecoder::new_with_options(ZCursor::new(IMAGE), options);
                let output = decoder.decode().unwrap();
                assert_eq!(decoder.dimensions(), Some((WIDTH, HEIGHT)));
                assert_native_pixels(
                    &output,
                    format.num_components(),
                    WIDTH * format.num_components()
                );
            }
        }
    }
}

#[test]
fn restart_read_ahead_preserves_raw_planes_and_padding() {
    for strict in [true, false] {
        let options = DecoderOptions::default().set_strict_mode(strict);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(IMAGE), options);
        decoder.decode_headers().unwrap();
        let mut raw = decoder.raw_output();
        assert_eq!(raw.num_components(), Some(1));
        let layout = raw.layout().unwrap();
        assert_eq!((layout[0].width, layout[0].height), (WIDTH, HEIGHT));
        let stride = WIDTH + 13;
        let mut output = vec![SENTINEL; stride * HEIGHT];
        raw.decode_into_planes_strided(&mut [output.as_mut_slice()], &[stride])
            .unwrap();
        assert_native_pixels(&output, 1, stride);
    }
}

#[test]
fn restart_read_ahead_preserves_scanlines_and_padding() {
    for strict in [true, false] {
        for rows_per_read in [1, 17] {
            let options = DecoderOptions::default()
                .set_strict_mode(strict)
                .jpeg_set_out_colorspace(ColorSpace::RGB);
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(IMAGE), options);
            let mut scanlines = decoder.scanline_output();
            assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
            assert_eq!(scanlines.output_row_bytes(), Some(WIDTH * 3));
            let stride = WIDTH * 3 + 13;
            let mut output = vec![SENTINEL; stride * HEIGHT];
            let mut rows = vec![SENTINEL; stride * rows_per_read];
            while scanlines.output_scanline() < HEIGHT {
                let start = scanlines.output_scanline();
                rows.fill(SENTINEL);
                match scanlines.read_scanlines(&mut rows, stride).unwrap() {
                    ScanlineReadStatus::RowsProcessed { rows: count } => {
                        assert!(count > 0 && count <= rows_per_read);
                        output[start * stride..(start + count) * stride]
                            .copy_from_slice(&rows[..count * stride]);
                        assert!(rows[count * stride..].iter().all(|&byte| byte == SENTINEL));
                    }
                    status => panic!("unexpected scanline status: {status:?}")
                }
            }
            assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
            assert_native_pixels(&output, 3, stride);
        }
    }
}
