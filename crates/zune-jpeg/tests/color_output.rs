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
use zune_jpeg::JpegDecoder;

const FIXTURES: [(&str, &[u8], (usize, usize)); 7] = [
    (
        "baseline 4:4:4",
        include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg"),
        (16, 16),
    ),
    (
        "baseline 4:2:2",
        include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg"),
        (64, 64),
    ),
    (
        "baseline 4:2:0",
        include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
        (64, 64),
    ),
    (
        "baseline odd-width SIMD tail",
        include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg"),
        (65, 65),
    ),
    (
        "progressive 4:4:4 SIMD tail",
        include_bytes!("../../../test-images/jpeg/progressive_dc_huffman_table_1.jpg"),
        (650, 470),
    ),
    (
        "progressive 4:2:0",
        include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg"),
        (256, 256),
    ),
    (
        "progressive odd-width SIMD tail",
        include_bytes!("../../../test-images/jpeg/synthetic_image.jpg"),
        (533, 800),
    ),
];

fn decode(data: &[u8], colorspace: ColorSpace) -> (Vec<u8>, (usize, usize)) {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let output = decoder.decode().unwrap();
    (output, decoder.dimensions().unwrap())
}

fn decode_into(data: &[u8], colorspace: ColorSpace) -> (Vec<u8>, (usize, usize)) {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    let dimensions = decoder.dimensions().unwrap();
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut output).unwrap();
    (output, dimensions)
}

#[test]
fn bgr_matches_rgb_with_red_and_blue_reversed() {
    for (name, data, expected_dimensions) in FIXTURES {
        let (rgb, rgb_dimensions) = decode(data, ColorSpace::RGB);
        let (bgr, bgr_dimensions) = decode_into(data, ColorSpace::BGR);

        assert_eq!(rgb_dimensions, expected_dimensions, "{name}");
        assert_eq!(bgr_dimensions, expected_dimensions, "{name}");
        assert_eq!(bgr.len(), rgb.len(), "{name}");
        for (pixel_index, (rgb, bgr)) in rgb.chunks_exact(3).zip(bgr.chunks_exact(3)).enumerate() {
            assert_eq!(bgr, [rgb[2], rgb[1], rgb[0]], "{name}: pixel {pixel_index}");
        }
    }
}

#[test]
fn bgra_matches_rgba_with_red_and_blue_reversed_and_opaque_alpha() {
    for (name, data, expected_dimensions) in FIXTURES {
        let (rgba, rgba_dimensions) = decode_into(data, ColorSpace::RGBA);
        let (bgra, bgra_dimensions) = decode(data, ColorSpace::BGRA);

        assert_eq!(rgba_dimensions, expected_dimensions, "{name}");
        assert_eq!(bgra_dimensions, expected_dimensions, "{name}");
        assert_eq!(bgra.len(), rgba.len(), "{name}");
        for (pixel_index, (rgba, bgra)) in
            rgba.chunks_exact(4).zip(bgra.chunks_exact(4)).enumerate()
        {
            assert_eq!(
                bgra,
                [rgba[2], rgba[1], rgba[0], 255],
                "{name}: pixel {pixel_index}"
            );
        }
    }
}

#[test]
fn luma_matches_y_of_ycbcr_for_vertically_sampled_images() {
    // Grayscale output of a vertically sampled image must be its Y channel, not other rows.
    let fixtures: [(&str, &[u8], (usize, usize)); 3] = [
        (
            "baseline non-interleaved 4:4:0",
            include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg"),
            (64, 64)
        ),
        (
            "progressive 4:4:0",
            include_bytes!("../../../test-images/jpeg/progressive_440_65x65.jpg"),
            (65, 65)
        ),
        (
            "progressive 4:2:0",
            include_bytes!("../../../test-images/jpeg/progressive_restart_420.jpg"),
            (256, 256)
        )
    ];
    for (name, data, expected_dimensions) in fixtures {
        let (luma, luma_dimensions) = decode(data, ColorSpace::Luma);
        let (ycbcr, _) = decode_into(data, ColorSpace::YCbCr);

        assert_eq!(luma_dimensions, expected_dimensions, "{name}");
        assert_eq!(luma.len() * 3, ycbcr.len(), "{name}");
        for (pixel_index, (luma, ycbcr)) in luma.iter().zip(ycbcr.chunks_exact(3)).enumerate() {
            assert_eq!(*luma, ycbcr[0], "{name}: pixel {pixel_index}");
        }
    }
}

#[test]
fn unsupported_lumaa_output_is_an_error_for_vertically_subsampled_images() {
    // 4:2:0: vertically subsampled chroma used to panic on carry-over of rows never decoded
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::LumaA);
    let err = JpegDecoder::new_with_options(ZCursor::new(data.as_slice()), options)
        .decode()
        .expect_err("LumaA output from YCbCr is not implemented");
    match err {
        DecodeErrors::Format(msg) => {
            assert_eq!(msg, "Unimplemented colorspace mapping from YCbCr to LumaA")
        }
        other => panic!("expected DecodeErrors::Format, got {other:?}")
    }
}

#[test]
fn generic_upsampling_preserves_delayed_boundary_rows() {
    let data = include_bytes!("../../../test-images/jpeg/issue_482_mixed_sampling.jpg");
    let reference = include_bytes!("../../../test-images/jpeg/issue_482_mixed_sampling.rgb");
    let (actual, dimensions) = decode(data, ColorSpace::RGB);

    assert_eq!(dimensions, (40, 24));
    assert_eq!(actual.len(), reference.len());

    let differences: Vec<u8> = actual
        .iter()
        .zip(reference)
        .map(|(actual, reference)| actual.abs_diff(*reference))
        .collect();
    let max_difference = differences.iter().copied().max().unwrap_or(0);
    let mean_difference =
        differences.iter().map(|difference| f64::from(*difference)).sum::<f64>()
            / differences.len() as f64;

    assert!(max_difference <= 8, "max channel difference {max_difference}");
    assert!(mean_difference < 3.0, "mean channel difference {mean_difference}");

    let row_bytes = 40 * 3;
    let delayed_rows = &differences[14 * row_bytes..16 * row_bytes];
    let delayed_max = delayed_rows.iter().copied().max().unwrap_or(0);
    assert!(
        delayed_max <= 8,
        "delayed MCU-boundary rows differ by {delayed_max}"
    );
}
