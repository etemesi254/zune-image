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
