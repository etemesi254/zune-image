/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(unused_variables, unused_imports)]
use std::fs::{read, File, OpenOptions};
use std::io::{Cursor, Write};
use std::path::Path;

use png::Transformations;
use zune_core::bytestream::ZCursor;
use zune_core::options::EncoderOptions;
use zune_png::{post_process_image, post_process_image_apng, FrameInfo, PngDecoder};

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
    read(path).unwrap()
}

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    let expand = Transformations::EXPAND;
    decoder.set_transformations(expand);

    let mut reader = decoder.read_info().unwrap();

    // Allocate the output buffer.
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    // Read the next frame. An APNG might contain multiple frames.
    let _ = reader.next_frame(&mut buf).unwrap();

    buf
}

fn decode_zune(data: &[u8]) -> Vec<u8> {
    PngDecoder::new(ZCursor::new(data)).decode_raw().unwrap()
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

#[test]
fn test_animation() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/animated_ball.png";
    let data = open_and_read(path);
    let mut decoder = PngDecoder::new(ZCursor::new(&data));
    decoder.decode_headers().unwrap();

    let colorspace = decoder.colorspace().unwrap();
    let depth = decoder.depth().unwrap();
    let info = decoder.info().unwrap().clone();

    let buffer_size = info.width * info.height * colorspace.num_components();

    // We now maintain the main output buffer AND a dedicated backup buffer for DisposeOp::Previous
    let mut output = vec![0; buffer_size];
    let mut canvas_backup = vec![0; buffer_size];
    let mut prev_frame_info: Option<FrameInfo> = None;

    while decoder.more_frames() {
        decoder.decode_headers().unwrap();
        // Clone frame_info so we can pass it around and store it for the next loop
        let frame = decoder.frame_info().unwrap();

        let pix = decoder.decode_raw().unwrap();
        let encoder_opts = EncoderOptions::new(info.width, info.height, colorspace, depth);

        post_process_image_apng(
            &info,
            colorspace,
            &frame,
            prev_frame_info.as_ref(),
            &pix,
            &mut canvas_backup,
            &mut output,
            None,
        )
        .unwrap();

        // let file = OpenOptions::new()
        //     .write(true)
        //     .truncate(true)
        //     .create(true)
        //     .open(format!("./{i}.png"))
        //     .unwrap();
        //
        // let c = std::io::BufWriter::new(file);
        //
        // // Encode and save the current state of the output canvas
        // zune_png::PngEncoder::new(&output, encoder_opts)
        //     .encode(c)
        //     .unwrap();

        // At the end of the loop, the current frame becomes the previous frame
        prev_frame_info = Some(frame);
        //i += 1;
    }
}

