/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(unused_variables, unused_imports)]
use std::fs::{read, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Write};
use std::path::Path;

use png::Transformations;
use zune_core::bytestream::ZCursor;
use zune_core::options::EncoderOptions;
use zune_png::{ApngContext, FrameInfo, PngDecoder};

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
    // where our output buffer will be written
    let mut output = vec![0; buffer_size];
    let mut ctx = ApngContext::<u8>::new(decoder.info().unwrap(), colorspace);

    let mut hash = std::hash::DefaultHasher::default();
    let expected_hash = 16516776064033238192_u64;
    while decoder.more_frames() {
        decoder.decode_headers().unwrap();
        let frame = decoder.frame_info().unwrap();

        let pix = decoder.decode_raw().unwrap();
        let encoder_opts = EncoderOptions::new(info.width, info.height, colorspace, depth);

        ctx.process_frame(&frame, pix.as_ref(), &mut output)
            .unwrap();

        output.hash(&mut hash);
    }
    assert_eq!(expected_hash, hash.finish());
}

#[test]
fn test_animation_clock() {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/clock.png";
    let data = open_and_read(path);
    let mut decoder = PngDecoder::new(ZCursor::new(&data));
    decoder.decode_headers().unwrap();

    let colorspace = decoder.colorspace().unwrap();
    let depth = decoder.depth().unwrap();
    let info = decoder.info().unwrap().clone();

    let buffer_size = info.width * info.height * colorspace.num_components();
    // where our output buffer will be written
    let mut output = vec![0; buffer_size];
    let mut ctx = ApngContext::<u8>::new(decoder.info().unwrap(), colorspace);

    let mut hash = std::hash::DefaultHasher::default();
    let expected_hash = 11331376798319734720_u64;
    while decoder.more_frames() {
        decoder.decode_headers().unwrap();
        let frame = decoder.frame_info().unwrap();

        let pix = decoder.decode_raw().unwrap();
        let encoder_opts = EncoderOptions::new(info.width, info.height, colorspace, depth);

        ctx.process_frame(&frame, pix.as_ref(), &mut output)
            .unwrap();

        output.hash(&mut hash);
    }
    assert_eq!(expected_hash, hash.finish());
}
