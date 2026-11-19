/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{JpegDecoder, JpegScale, NeverCancel, ScanlineReadStatus, ScanlineStatus};

const SCALES: [JpegScale; 7] = [
    JpegScale::Half,
    JpegScale::Quarter,
    JpegScale::Eighth,
    JpegScale::ThreeEighths,
    JpegScale::FiveEighths,
    JpegScale::ThreeQuarters,
    JpegScale::SevenEighths
];

fn check_native_output(data: &[u8], expected: [u64; 7]) {
    for use_simd in [false, true] {
        let options = DecoderOptions::default().set_use_unsafe(use_simd);
        check_native_output_with_options(data, expected, options);
    }
}

fn check_native_output_with_options(data: &[u8], expected: [u64; 7], options: DecoderOptions) {
    for (scale, digest) in SCALES.into_iter().zip(expected) {
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
        decoder.decode_headers().unwrap();
        let info = decoder.info().unwrap();
        let input_colorspace = decoder.input_colorspace().unwrap();
        let dimensions = (
            (usize::from(info.width) * scale.numerator()).div_ceil(scale.denominator()),
            (usize::from(info.height) * scale.numerator()).div_ceil(scale.denominator())
        );
        assert_eq!(decoder.scaled_dimensions(scale), Some(dimensions));
        let length = decoder.scaled_output_buffer_size(scale).unwrap();
        assert_eq!(length, dimensions.0 * dimensions.1 * 3);
        let mut output = vec![0xa5; length + 5];
        assert!(matches!(
            decoder.decode_scaled_into(scale, &mut output[..length - 1]),
            Err(DecodeErrors::TooSmallOutput(..))
        ));
        assert!(output.iter().all(|&byte| byte == 0xa5));
        decoder.decode_scaled_into(scale, &mut output).unwrap();
        assert_eq!(&output[length..], &[0xa5; 5]);
        let actual = output[..length]
            .iter()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
        assert_eq!(actual, digest, "native pixel digest for {scale:?}");
        let original = output.clone();
        output[..length].fill(0);
        decoder.decode_scaled_into(scale, &mut output).unwrap();
        assert_eq!(output, original);

        for format in [ColorSpace::RGBA, ColorSpace::BGR, ColorSpace::BGRA] {
            let options = options.jpeg_set_out_colorspace(format);
            let mut packed = JpegDecoder::new_with_options(ZCursor::new(data), options);
            packed.decode_headers().unwrap();
            let channels = format.num_components();
            let packed_length = dimensions.0 * dimensions.1 * channels;
            assert_eq!(packed.scaled_output_buffer_size(scale), Some(packed_length));
            let mut pixels = vec![0xa5; packed_length + 5];
            assert!(matches!(
                packed.decode_scaled_into(scale, &mut pixels[..packed_length - 1]),
                Err(DecodeErrors::TooSmallOutput(..))
            ));
            assert!(pixels.iter().all(|&byte| byte == 0xa5));
            if input_colorspace == ColorSpace::Luma
                && matches!(format, ColorSpace::BGR | ColorSpace::BGRA)
            {
                let error = packed.decode_scaled_into(scale, &mut pixels).unwrap_err();
                match error {
                    DecodeErrors::Format(message) => assert_eq!(
                        message,
                        format!("Unimplemented colorspace mapping from Luma to {format:?}")
                    ),
                    other => panic!("expected unsupported colorspace error, got {other:?}")
                }
                assert!(pixels.iter().all(|&byte| byte == 0xa5));
                continue;
            }
            packed.decode_scaled_into(scale, &mut pixels).unwrap();
            assert_eq!(&pixels[packed_length..], &[0xa5; 5]);
            for (rgb, pixel) in output[..length]
                .chunks_exact(3)
                .zip(pixels[..packed_length].chunks_exact(channels))
            {
                let expected = if matches!(format, ColorSpace::BGR | ColorSpace::BGRA) {
                    [rgb[2], rgb[1], rgb[0], 255]
                } else {
                    [rgb[0], rgb[1], rgb[2], 255]
                };
                assert_eq!(pixel, &expected[..channels], "{scale:?}, {format:?}");
            }
            let expected = pixels.clone();
            pixels[..packed_length].fill(0);
            packed.decode_scaled_into(scale, &mut pixels).unwrap();
            assert_eq!(pixels, expected);
        }
    }
}

#[test]
fn scaled_output_preserves_geometry_and_replays_across_output_modes() {
    let fixtures: [&[u8]; 3] = [
        include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
        include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg"),
        include_bytes!("../../../test-images/jpeg/2029.jpg")
    ];
    for data in fixtures {
        let mut reference = JpegDecoder::new(ZCursor::new(data));
        let full = reference.decode().unwrap();
        let mut decoder = JpegDecoder::new(ZCursor::new(data));
        assert_eq!(decoder.scaled_dimensions(JpegScale::Half), None);
        assert_eq!(decoder.scaled_output_buffer_size(JpegScale::Half), None);
        decoder.decode_headers().unwrap();
        let info = decoder.info().unwrap();
        let full_length = decoder.output_buffer_size().unwrap();
        for scale in [JpegScale::Half, JpegScale::SevenEighths, JpegScale::Quarter] {
            let length = decoder.scaled_output_buffer_size(scale).unwrap();
            let mut reduced = vec![0; length];
            decoder.decode_scaled_into(scale, &mut reduced).unwrap();
            let mut fresh = JpegDecoder::new(ZCursor::new(data));
            let mut expected = vec![0; length];
            fresh.decode_scaled_into(scale, &mut expected).unwrap();
            assert_eq!(reduced, expected);
            assert_eq!(decoder.info().unwrap().width, info.width);
            assert_eq!(decoder.info().unwrap().height, info.height);
            assert_eq!(decoder.output_buffer_size(), Some(full_length));
            assert_eq!(decoder.decoded_output_bytes(), Some(0));
            assert_eq!(decoder.decoded_scanlines(), Some(0));
            if decoder.decoded_preview_output_bytes().is_some() {
                assert_eq!(decoder.decoded_preview_output_bytes(), Some(0));
            }
        }

        assert_eq!(decoder.decode().unwrap(), full);
        let scale = JpegScale::Half;
        let mut reduced = vec![0; decoder.scaled_output_buffer_size(scale).unwrap()];
        decoder.decode_scaled_into(scale, &mut reduced).unwrap();
        {
            let mut raw = decoder.raw_output();
            let mut reference_raw = reference.raw_output();
            let layout = raw.layout().unwrap();
            assert_eq!(layout, reference_raw.layout().unwrap());
            let count = raw.num_components().unwrap();
            let mut planes: Vec<Vec<u8>> = layout[..count]
                .iter()
                .map(|plane| vec![0; plane.byte_size])
                .collect();
            let mut expected = planes.clone();
            raw.decode_into_planes(
                &mut planes.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>()
            )
            .unwrap();
            reference_raw
                .decode_into_planes(
                    &mut expected
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>()
                )
                .unwrap();
            assert_eq!(planes, expected);
        }
        decoder.decode_scaled_into(scale, &mut reduced).unwrap();
        {
            let mut scanlines = decoder.scanline_output();
            assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
            let row_bytes = scanlines.output_row_bytes().unwrap();
            assert_eq!(scanlines.output_height(), Some(usize::from(info.height)));
            let mut pixels = vec![0; full_length];
            for row in pixels.chunks_exact_mut(row_bytes) {
                assert_eq!(
                    scanlines.read_scanlines(row, row_bytes).unwrap(),
                    ScanlineReadStatus::RowsProcessed { rows: 1 }
                );
            }
            assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
            assert_eq!(pixels, full);
        }
        let mut replayed = vec![0; reduced.len()];
        decoder.decode_scaled_into(scale, &mut replayed).unwrap();
        assert_eq!(replayed, reduced);
    }
}

#[test]
fn scaled_option_changes_invalidate_cached_output() {
    let data = include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(data.as_slice()));
    decoder.decode_headers().unwrap();
    let scale = JpegScale::Half;
    let mut rgb = vec![0; decoder.scaled_output_buffer_size(scale).unwrap()];
    decoder.decode_scaled_into(scale, &mut rgb).unwrap();
    let options = DecoderOptions::default()
        .set_use_unsafe(false)
        .jpeg_set_out_colorspace(ColorSpace::RGBA);
    decoder.set_options(options);
    let mut output = vec![0; decoder.scaled_output_buffer_size(scale).unwrap()];
    decoder.decode_scaled_into(scale, &mut output).unwrap();
    let mut reference = JpegDecoder::new_with_options(ZCursor::new(data.as_slice()), options);
    let mut expected = vec![0; output.len()];
    reference.decode_scaled_into(scale, &mut expected).unwrap();
    assert_eq!(output, expected);
}

#[test]
fn scaled_cancellation_retains_scan_and_cached_frame_state() {
    let fixtures: [&[u8]; 2] = [
        include_bytes!("../../../test-images/jpeg/2029.jpg"),
        include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg")
    ];
    for data in fixtures {
        let mut decoder = JpegDecoder::new(ZCursor::new(data));
        decoder.decode_headers().unwrap();
        decoder.set_incremental_mode(true);
        decoder.set_cancel_interval(1);
        let polls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&polls);
        decoder.set_cancel(move || observed.fetch_add(1, Ordering::Relaxed) >= 1);
        let scale = JpegScale::Half;
        let mut output = vec![0xa5; decoder.scaled_output_buffer_size(scale).unwrap()];
        assert!(matches!(
            decoder.decode_scaled_into(scale, &mut output),
            Err(DecodeErrors::Cancelled)
        ));
        assert!(output.iter().all(|&byte| byte == 0xa5));
        assert!(polls.load(Ordering::Relaxed) >= 2);
        decoder.set_cancel(NeverCancel);
        decoder.decode_scaled_into(scale, &mut output).unwrap();
        let mut reference = JpegDecoder::new(ZCursor::new(data));
        let mut expected = vec![0; output.len()];
        reference.decode_scaled_into(scale, &mut expected).unwrap();
        assert_eq!(output, expected);

        decoder.set_cancel(|| true);
        output.fill(0xa5);
        assert!(matches!(
            decoder.decode_scaled_into(scale, &mut output),
            Err(DecodeErrors::Cancelled)
        ));
        decoder.set_cancel(NeverCancel);
        decoder.decode_scaled_into(scale, &mut output).unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn scaled_non_interleaved_native_parity() {
    // Digests are from native JDCT_ISLOW decoding, not resized full-size pixels.
    check_native_output(
        include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg"),
        [
            0x284b2e337892e025,
            0xbd32a3785fdb3385,
            0xff130fe90a5d7dfd,
            0x9dc7a457ff19e63d,
            0x82075bd236e4caed,
            0xc8dd4e9ed42c61d5,
            0x1f1a6f5c9390b3bd
        ]
    );
    check_native_output(
        include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
        [
            0x6ef571d11f2bf9c5,
            0x890590b48c9f1ea5,
            0x317f1eb000afaf75,
            0x4aaf3f79b73321fd,
            0xef22f61bced45c15,
            0x1ad781120a196935,
            0xd0d50e5969fb42f5
        ]
    );
    check_native_output(
        include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg"),
        [
            0x284b2e337892e025,
            0xbd32a3785fdb3385,
            0xff130fe90a5d7dfd,
            0x9dc7a457ff19e63d,
            0x82075bd236e4caed,
            0xc8dd4e9ed42c61d5,
            0x1f1a6f5c9390b3bd
        ]
    );
    check_native_output(
        include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg"),
        [
            0xbef955b26594fd25,
            0xcce22cb770e95bc5,
            0x3b439a302cad73c5,
            0xe623396d704da475,
            0x8e15a5a513096205,
            0xfc1de57ae12b66a5,
            0x0acdc771aebe93cd
        ]
    );
}

#[test]
fn scaled_interleaved_scalar_chroma_matches_dispatched() {
    // Half-scale 4:2:0 uses full-size chroma IDCTs. Scalar transforms must
    // not overwrite the entropy decoder's reusable sparse coefficients.
    let data = include_bytes!("../../../test-images/jpeg/2029.jpg");
    for scale in SCALES {
        let decode = |use_simd| {
            let options = DecoderOptions::default().set_use_unsafe(use_simd);
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data.as_slice()), options);
            decoder.decode_headers().unwrap();
            assert_eq!(
                decoder.scaled_dimensions(scale),
                Some((
                    (388 * scale.numerator()).div_ceil(scale.denominator()),
                    (477 * scale.numerator()).div_ceil(scale.denominator())
                ))
            );
            let mut pixels = vec![0; decoder.scaled_output_buffer_size(scale).unwrap()];
            decoder.decode_scaled_into(scale, &mut pixels).unwrap();
            pixels
        };
        let scalar = decode(false);
        let dispatched = decode(true);
        assert_eq!(scalar.len(), dispatched.len());
        assert_eq!(
            scalar.iter().zip(&dispatched).position(|(a, b)| a != b),
            None,
            "scalar/dispatched chroma mismatch at {scale:?}"
        );
    }
}

#[test]
fn scaled_progressive_restart_native_parity() {
    check_native_output(
        include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg"),
        [
            0xbd24891575162344,
            0x7bc0dcf5e6f205c9,
            0x9465979da1ca8b5a,
            0x0dad861cc9c8159f,
            0x672156c2890f6319,
            0x5baef2d28fb38110,
            0x0ceacba608666f57
        ]
    );
}

#[test]
fn scaled_odd_dimensions_and_grayscale_sampling_native_parity() {
    check_native_output(
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
        [
            0x399b2ae253ccfb2e,
            0x813a956887962950,
            0xf19f6c5e6f45d8f4,
            0x4c9ff31cfb30ecec,
            0xd48b8104691446cc,
            0xb73a9cbd3c246ddd,
            0xfc97a1afdb3f367d
        ]
    );
}
