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
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{JpegDecoder, JpegScale};

fn check_native_output(data: &[u8], expected: [u64; 3]) {
    for use_simd in [false, true] {
        let options = DecoderOptions::default().set_use_unsafe(use_simd);
        check_native_output_with_options(data, expected, options);
    }
}

fn check_native_output_with_options(data: &[u8], expected: [u64; 3], options: DecoderOptions) {
    for (scale, digest) in [JpegScale::Half, JpegScale::Quarter, JpegScale::Eighth]
        .into_iter()
        .zip(expected)
    {
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
        decoder.decode_headers().unwrap();
        let info = decoder.info().unwrap();
        let input_colorspace = decoder.input_colorspace().unwrap();
        let dimensions = (
            usize::from(info.width).div_ceil(scale.denominator()),
            usize::from(info.height).div_ceil(scale.denominator())
        );
        let mut decoder = decoder.into_scaled(scale).unwrap();
        assert_eq!(decoder.dimensions(), dimensions);
        let length = decoder.output_buffer_size().unwrap();
        assert_eq!(length, dimensions.0 * dimensions.1 * 3);
        let mut output = vec![0xa5; length + 5];
        assert!(matches!(
            decoder.decode_into(&mut output[..length - 1]),
            Err(DecodeErrors::TooSmallOutput(..))
        ));
        assert!(output.iter().all(|&byte| byte == 0xa5));
        decoder.decode_into(&mut output).unwrap();
        assert_eq!(&output[length..], &[0xa5; 5]);
        let actual = output[..length]
            .iter()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
        assert_eq!(actual, digest, "native pixel digest for {scale:?}");
        let original = output.clone();
        output[..length].fill(0);
        decoder.decode_into(&mut output).unwrap();
        assert_eq!(output, original);

        for format in [ColorSpace::RGBA, ColorSpace::BGR, ColorSpace::BGRA] {
            let options = options.jpeg_set_out_colorspace(format);
            let mut packed = JpegDecoder::new_with_options(ZCursor::new(data), options)
                .into_scaled(scale)
                .unwrap();
            let channels = format.num_components();
            let packed_length = dimensions.0 * dimensions.1 * channels;
            assert_eq!(packed.output_buffer_size(), Some(packed_length));
            let mut pixels = vec![0xa5; packed_length + 5];
            assert!(matches!(
                packed.decode_into(&mut pixels[..packed_length - 1]),
                Err(DecodeErrors::TooSmallOutput(..))
            ));
            assert!(pixels.iter().all(|&byte| byte == 0xa5));
            if input_colorspace == ColorSpace::Luma
                && matches!(format, ColorSpace::BGR | ColorSpace::BGRA)
            {
                let error = packed.decode_into(&mut pixels).unwrap_err();
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
            packed.decode_into(&mut pixels).unwrap();
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
            packed.decode_into(&mut pixels).unwrap();
            assert_eq!(pixels, expected);
        }
    }
}

#[test]
fn scaled_selection_after_scan_decode_is_rejected() {
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(data.as_slice()));
    decoder.decode().unwrap();
    assert!(matches!(
        decoder.into_scaled(JpegScale::Half),
        Err(DecodeErrors::FormatStatic(
            "select output scaling before starting scan decoding"
        ))
    ));
}

#[test]
fn scaled_non_interleaved_native_parity() {
    // Digests are from native JDCT_ISLOW decoding, not resized full-size pixels.
    check_native_output(
        include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg"),
        [0x284b2e337892e025, 0xbd32a3785fdb3385, 0xff130fe90a5d7dfd]
    );
    check_native_output(
        include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
        [0x6ef571d11f2bf9c5, 0x890590b48c9f1ea5, 0x317f1eb000afaf75]
    );
    check_native_output(
        include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg"),
        [0x284b2e337892e025, 0xbd32a3785fdb3385, 0xff130fe90a5d7dfd]
    );
    check_native_output(
        include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg"),
        [0xbef955b26594fd25, 0xcce22cb770e95bc5, 0x3b439a302cad73c5]
    );
}

#[test]
fn scaled_interleaved_scalar_chroma_matches_dispatched() {
    // Half-scale 4:2:0 uses full-size chroma IDCTs. Scalar transforms must
    // not overwrite the entropy decoder's reusable sparse coefficients.
    let data = include_bytes!("../../../test-images/jpeg/2029.jpg");
    for scale in [JpegScale::Half, JpegScale::Quarter, JpegScale::Eighth] {
        let decode = |use_simd| {
            let options = DecoderOptions::default().set_use_unsafe(use_simd);
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data.as_slice()), options)
                .into_scaled(scale)
                .unwrap();
            assert_eq!(
                decoder.dimensions(),
                (
                    388_usize.div_ceil(scale.denominator()),
                    477_usize.div_ceil(scale.denominator())
                )
            );
            let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
            decoder.decode_into(&mut pixels).unwrap();
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
        [0xbd24891575162344, 0x7bc0dcf5e6f205c9, 0x9465979da1ca8b5a]
    );
}

#[test]
fn scaled_odd_dimensions_and_grayscale_sampling_native_parity() {
    check_native_output(
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
        [0x399b2ae253ccfb2e, 0x813a956887962950, 0xf19f6c5e6f45d8f4]
    );
}
