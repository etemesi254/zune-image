/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! A PLTE chunk in a non-indexed image must not change how the image decodes.

use std::fs::read;
use std::io::Cursor;

use zune_core::bytestream::ZCursor;

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();

    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let _ = reader.next_frame(&mut buf).unwrap();

    buf
}

fn decode_zune(data: &[u8]) -> Vec<u8> {
    zune_png::PngDecoder::new(ZCursor::new(data))
        .decode_raw()
        .unwrap()
}

#[test]
fn test_truecolor_with_suggested_palette() {
    // A PLTE chunk in a truecolor image is only a suggested palette
    for name in ["pp0n2c16.png", "pp0n6a08.png"] {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/" + name;
        let contents = read(path).unwrap();

        assert_eq!(decode_zune(&contents), decode_ref(&contents), "{name}");
    }
}

#[test]
fn test_low_bit_grayscale_with_palette() {
    // A PLTE chunk in a grayscale image is not allowed, but it must not stop the
    // samples from being scaled to 8 bits
    for depth in [png::BitDepth::One, png::BitDepth::Two, png::BitDepth::Four] {
        let mut contents = Vec::new();
        let mut encoder = png::Encoder::new(&mut contents, 7, 3);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(depth);
        encoder.set_palette(vec![0, 255, 0, 255, 0, 0]);
        let mut writer = encoder.write_header().unwrap();
        let row = (7 * depth as usize).div_ceil(8);
        let data: Vec<u8> = (0..3 * row).map(|i| (i as u8).wrapping_mul(0x5b)).collect();
        writer.write_image_data(&data).unwrap();
        writer.finish().unwrap();

        assert_eq!(
            decode_zune(&contents),
            decode_ref(&contents),
            "depth {depth:?}"
        );
    }
}
