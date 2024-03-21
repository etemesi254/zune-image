/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::path::Path;

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_png::PngDecoder;

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
    read(path).unwrap()
}

fn test_decoding<P: AsRef<Path>>(path: P) {
    let contents = open_and_read(path);
    let options = DecoderOptions::default().png_set_add_alpha_channel(true);
    let mut decoder = PngDecoder::new_with_options(ZCursor::new(&contents), options);
    let pixels = decoder.decode_raw().unwrap();

    assert!(decoder.colorspace().unwrap().has_alpha());
    let (width, height) = decoder.dimensions().unwrap();
    let colorspace = decoder.colorspace().unwrap();
    let depth = decoder.depth().unwrap();

    assert_eq!(
        pixels.len(),
        width * height * colorspace.num_components() * depth.size_of()
    );

    // check for 255

    if depth == BitDepth::Eight {
        for ch in pixels.chunks_exact(4) {
            assert_eq!(ch[3], 255);
        }
    } else if depth == BitDepth::Sixteen {
        for ch in pixels.chunks_exact(8) {
            assert_eq!(ch[6], 255);
            assert_eq!(ch[7], 255);
        }
    }
}

#[test]
fn test_rgb_to_rgba() {
    // non interlaced 1bpp
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/f01n0g08.png";
    test_decoding(path);

    // 16 bit RGB
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/pp0n2c16.png";
    test_decoding(path);

    // 2 bit palette
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi3p02.png";
    test_decoding(path);

    // 8 bit palette
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/basi3p08.png";
    test_decoding(path);
}
