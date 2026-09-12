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

// Fixtures are CC0-1.0 JPEGs copied from robert-ancell/jpegsuite revision
// 8382e7831896cd1adf1cc61a1c9e565c4030aa43.
const RGB_OUTPUT_SIZE: usize = 32 * 32 * 3;

const BASELINE_RGB_FIXTURES: [(&str, &[u8]); 2] = [
    (
        "baseline_rgb",
        include_bytes!("../../../test-images/jpeg/app14/baseline_rgb.jpg")
    ),
    (
        "baseline_rgb_interleaved",
        include_bytes!("../../../test-images/jpeg/app14/baseline_rgb_interleaved.jpg")
    )
];

const EXTENDED_RGB_FIXTURES: [(&str, &[u8]); 2] = [
    (
        "extended_huffman_rgb",
        include_bytes!("../../../test-images/jpeg/app14/extended_huffman_rgb.jpg")
    ),
    (
        "extended_huffman_rgb_interleaved",
        include_bytes!("../../../test-images/jpeg/app14/extended_huffman_rgb_interleaved.jpg")
    )
];

const PROGRESSIVE_RGB_FIXTURES: [(&str, &[u8]); 2] = [
    (
        "progressive_huffman_rgb",
        include_bytes!("../../../test-images/jpeg/app14/progressive_huffman_rgb.jpg")
    ),
    (
        "progressive_huffman_rgb_interleaved",
        include_bytes!("../../../test-images/jpeg/app14/progressive_huffman_rgb_interleaved.jpg")
    )
];

fn assert_rgb_headers(name: &str, data: &[u8]) {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder
        .decode_headers()
        .unwrap_or_else(|error| panic!("{name}: header decode failed: {error:?}"));

    assert_eq!(decoder.dimensions(), Some((32, 32)), "{name}");
    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::RGB), "{name}");
    assert_eq!(
        decoder.output_buffer_size(),
        Some(RGB_OUTPUT_SIZE),
        "{name}"
    );
}

fn assert_rgb_decode(name: &str, data: &[u8]) {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder
        .decode_headers()
        .unwrap_or_else(|error| panic!("{name}: header decode failed: {error:?}"));
    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::RGB), "{name}");

    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    decoder
        .decode_into(&mut output)
        .unwrap_or_else(|error| panic!("{name}: RGB decode failed: {error:?}"));

    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::RGB), "{name}");
    assert_eq!(output.len(), RGB_OUTPUT_SIZE, "{name}");
}

fn assert_metadata_selected_decode(name: &str, data: &[u8]) {
    let mut metadata_decoder = JpegDecoder::new(ZCursor::new(data));
    metadata_decoder
        .decode_headers()
        .unwrap_or_else(|error| panic!("{name}: metadata header decode failed: {error:?}"));
    let reported = metadata_decoder.input_colorspace().unwrap();
    assert_eq!(reported, ColorSpace::RGB, "{name}");

    let options = DecoderOptions::default().jpeg_set_out_colorspace(reported);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let output = decoder
        .decode()
        .unwrap_or_else(|error| panic!("{name}: metadata-selected decode failed: {error:?}"));

    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::RGB), "{name}");
    assert_eq!(output.len(), RGB_OUTPUT_SIZE, "{name}");
}

fn with_adobe_transform(data: &[u8], transform: u8) -> Vec<u8> {
    let mut result = data.to_vec();
    if let Some(offset) = result.windows(2).position(|bytes| bytes == [0xFF, 0xEE]) {
        let body_start = offset + 4;
        assert_eq!(
            result.get(body_start..body_start + 5),
            Some(b"Adobe".as_slice())
        );
        result[body_start + 11] = transform;
        return result;
    }

    let mut app14 = vec![0xFF, 0xEE, 0x00, 0x0E];
    app14.extend_from_slice(b"Adobe");
    app14.extend_from_slice(&[0x00, 0x64, 0x00, 0x00, 0x00, 0x00, transform]);
    result.splice(2..2, app14);
    result
}

fn with_component_ids(data: &[u8], ids: &[u8]) -> Vec<u8> {
    let mut result = data.to_vec();
    let sof = result
        .windows(2)
        .position(|bytes| bytes[0] == 0xFF && (0xC0..=0xC2).contains(&bytes[1]))
        .expect("fixture must contain a supported SOF marker");
    assert_eq!(usize::from(result[sof + 9]), ids.len());
    let old_ids: Vec<_> = (0..ids.len())
        .map(|index| result[sof + 10 + 3 * index])
        .collect();

    for (index, id) in ids.iter().enumerate() {
        result[sof + 10 + 3 * index] = *id;
    }
    for sos in result
        .windows(2)
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == [0xFF, 0xDA]).then_some(offset))
        .collect::<Vec<_>>()
    {
        for index in 0..usize::from(result[sos + 4]) {
            let selector = &mut result[sos + 5 + 2 * index];
            if let Some(id_index) = old_ids.iter().position(|id| id == selector) {
                *selector = ids[id_index];
            }
        }
    }
    result
}

fn app14_after_sof(data: &[u8]) -> Vec<u8> {
    let mut result = data.to_vec();
    let app14 = result
        .windows(2)
        .position(|bytes| bytes == [0xFF, 0xEE])
        .expect("fixture must contain APP14");
    let app14_len = 2 + usize::from(u16::from_be_bytes([result[app14 + 2], result[app14 + 3]]));
    let segment: Vec<_> = result.drain(app14..app14 + app14_len).collect();

    let sof = result
        .windows(2)
        .position(|bytes| bytes[0] == 0xFF && (0xC0..=0xC2).contains(&bytes[1]))
        .expect("fixture must contain a supported SOF marker");
    let sof_end = sof + 2 + usize::from(u16::from_be_bytes([result[sof + 2], result[sof + 3]]));
    result.splice(sof_end..sof_end, segment);
    result
}

#[test]
fn transform_zero_rgb_is_final_after_headers() {
    for (name, data) in BASELINE_RGB_FIXTURES
        .into_iter()
        .chain(EXTENDED_RGB_FIXTURES)
        .chain(PROGRESSIVE_RGB_FIXTURES)
    {
        assert_rgb_headers(name, data);
    }
}

#[test]
fn baseline_and_extended_transform_zero_rgb_decode() {
    for (name, data) in BASELINE_RGB_FIXTURES
        .into_iter()
        .chain(EXTENDED_RGB_FIXTURES)
    {
        assert_rgb_decode(name, data);
    }
}

#[test]
fn progressive_transform_zero_rgb_decodes_in_non_strict_mode() {
    for (name, data) in PROGRESSIVE_RGB_FIXTURES {
        assert_rgb_decode(name, data);
    }
}

#[test]
fn metadata_selected_rgb_output_decodes() {
    for (name, data) in BASELINE_RGB_FIXTURES
        .into_iter()
        .chain(EXTENDED_RGB_FIXTURES)
    {
        assert_metadata_selected_decode(name, data);
    }
}

#[test]
fn four_component_transform_zero_remains_cmyk() {
    let data = include_bytes!("../../../test-images/jpeg/app14/baseline_cmyk.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    assert_eq!(decoder.dimensions(), Some((32, 32)));
    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::CMYK));

    let output = decoder.decode().unwrap();
    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::CMYK));
    assert_eq!(output.len(), RGB_OUTPUT_SIZE);
}

#[test]
fn ycck_decodes_to_cmyk_without_inverting() {
    // Adobe stores YCCK with CMY inverted, so decoding to CMYK has to undo that. libjpeg reads
    // this fixture as CMYK [240, 58, 21, 216]; before YCCK to CMYK existed the only way out was
    // RGB, which returns the inverted tone.
    let data = include_bytes!("../../../test-images/jpeg/four_components.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::YCCK));

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::CMYK);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let output = decoder.decode().expect("YCCK to CMYK decode failed");

    let pixels = output.len() / 4;
    assert_eq!(pixels, 1318 * 611);
    let mean = |channel: usize| {
        output.chunks_exact(4).map(|p| u64::from(p[channel])).sum::<u64>() / pixels as u64
    };
    // A light image: little magenta or yellow, heavy cyan and black.
    assert_eq!([mean(0), mean(1), mean(2), mean(3)], [240, 58, 21, 216]);
}

#[test]
fn transform_one_is_ycbcr_and_transform_two_is_ycck() {
    let transform_one = with_adobe_transform(
        include_bytes!("../../../test-images/jpeg/app14/baseline_ycbcr.jpg"),
        1
    );
    let transform_two = with_adobe_transform(
        include_bytes!("../../../test-images/jpeg/app14/baseline_cmyk.jpg"),
        2
    );

    for (name, data, expected) in [
        ("transform_one", transform_one, ColorSpace::YCbCr),
        ("transform_two", transform_two, ColorSpace::YCCK)
    ] {
        let mut decoder = JpegDecoder::new(ZCursor::new(&data));
        decoder.decode_headers().unwrap();
        assert_eq!(decoder.input_colorspace(), Some(expected), "{name}");
        let output = decoder.decode().unwrap();
        assert_eq!(decoder.input_colorspace(), Some(expected), "{name}");
        assert_eq!(output.len(), RGB_OUTPUT_SIZE, "{name}");
    }
}

#[test]
fn app14_overrides_jfif_and_component_id_heuristics() {
    let jfif_with_rgb_transform = with_adobe_transform(
        include_bytes!("../../../test-images/jpeg/app14/baseline_ycbcr.jpg"),
        0
    );
    let rgb_ids_with_ycbcr_transform = with_component_ids(
        &with_adobe_transform(
            include_bytes!("../../../test-images/jpeg/app14/baseline_rgb_interleaved.jpg"),
            1
        ),
        b"RGB"
    );

    assert_rgb_headers(
        "APP14 transform zero overrides JFIF",
        &jfif_with_rgb_transform
    );

    let mut decoder = JpegDecoder::new(ZCursor::new(&rgb_ids_with_ycbcr_transform));
    decoder.decode_headers().unwrap();
    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::YCbCr));
}

#[test]
fn app14_marker_order_does_not_change_rgb_classification() {
    let before_sof = include_bytes!("../../../test-images/jpeg/app14/baseline_rgb.jpg");
    let after_sof = app14_after_sof(before_sof);

    assert_rgb_decode("APP14 before SOF", before_sof);
    assert_rgb_decode("APP14 after SOF", &after_sof);
}
