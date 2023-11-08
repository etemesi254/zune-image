/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::path::Path;

use png::Transformations;
use zune_core::options::EncoderOptions;
use zune_png::{post_process_image, PngDecoder};

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
    read(path).unwrap()
}

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(data);
    let expand = Transformations::EXPAND;
    decoder.set_transformations(expand);

    let mut reader = decoder.read_info().unwrap();

    // Allocate the output buffer.
    let mut buf = vec![0; reader.output_buffer_size()];
    // Read the next frame. An APNG might contain multiple frames.
    let _ = reader.next_frame(&mut buf).unwrap();

    buf
}

fn decode_zune(data: &[u8]) -> Vec<u8> {
    zune_png::PngDecoder::new(data).decode_raw().unwrap()
}

fn test_decoding<P: AsRef<Path>>(path: P) {
    let contents = open_and_read(path);

    let zune_results = decode_zune(&contents);
    let ref_results = decode_ref(&contents);
    assert_eq!(&zune_results, &ref_results);
}

#[test]
fn test_trns_transparency() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/xc.png";

    test_decoding(path);
}
// DISABLED FOR NOW

#[test]
fn test_animation() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/animated_ball.png";
    let data = open_and_read(path);
    let mut decoder = PngDecoder::new(&data);
    decoder.decode_headers().unwrap();
    let colorspace = decoder.get_colorspace().unwrap();
    let depth = decoder.get_depth().unwrap();
    let mut i = 0;
    let info = decoder.get_info().unwrap().clone();
    let mut background: Option<Vec<u8>> = None;
    let mut output =
        vec![0; info.width * info.height * decoder.get_colorspace().unwrap().num_components()];

    while decoder.more_frames() {
        decoder.decode_headers().unwrap();
        let frame = decoder.frame_info().unwrap();

        let pix = decoder.decode_raw().unwrap();
        let encoder_opts = EncoderOptions::new(info.width, info.height, colorspace, depth);
        post_process_image(
            &info,
            colorspace,
            &frame,
            &pix,
            background.as_ref().map(|x| x.as_slice()),
            &mut output,
            None
        )
        .unwrap();

        let bytes = zune_png::PngEncoder::new(&output, encoder_opts).encode();

        std::fs::write(format!("./{i}.png"), bytes).unwrap();
        background = Some(pix);
        i += 1;
    }
}

#[test]
fn test_animation_2() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/030.png";
    let data = open_and_read(path);
    let mut decoder = PngDecoder::new(&data);
    decoder.decode_headers().unwrap();
    let c = decoder.is_animated();
    let colorspace = decoder.get_colorspace().unwrap();
    let depth = decoder.get_depth().unwrap();
    let mut i = 0;
    let info = decoder.get_info().unwrap().clone();
    let mut background: Option<Vec<u8>> = None;
    let mut output =
        vec![0; info.width * info.height * decoder.get_colorspace().unwrap().num_components()];

    while decoder.more_frames() {
        decoder.decode_headers().unwrap();
        let frame = decoder.frame_info().unwrap();

        let pix = decoder.decode_raw().unwrap();
        let encoder_opts = EncoderOptions::new(info.width, info.height, colorspace, depth);
        post_process_image(
            &info,
            colorspace,
            &frame,
            &pix,
            background.as_ref().map(|x| x.as_slice()),
            &mut output,
            None
        )
        .unwrap();

        let bytes = zune_png::PngEncoder::new(&output, encoder_opts).encode();

        std::fs::write(format!("./{i}.png"), bytes).unwrap();
        background = Some(pix);
        i += 1;
    }
}
