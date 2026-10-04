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

/// Frames the `more_frames()` loop yields that are part of the animation.
fn animation_frames(data: &[u8]) -> usize {
    let mut decoder = PngDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    let mut frames = 0;
    while decoder.more_frames() {
        decoder.decode_headers().unwrap();
        let frame = decoder.frame_info().unwrap();
        decoder.decode_raw().unwrap();
        frames += usize::from(frame.is_part_of_seq);
    }
    frames
}

#[test]
fn more_frames_yields_every_frame_in_actl() {
    // acTL num_frames: animated_ball 20 and clock 40 (default image is the first frame),
    // 030 2 (separate default image, then 2 frames).
    for (name, frames) in [("animated_ball.png", 20), ("clock.png", 40), ("030.png", 2)] {
        let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/" + name;
        assert_eq!(animation_frames(&open_and_read(path)), frames, "{name}");
    }
}

#[test]
fn more_frames_stops_when_the_input_ends() {
    // An animated PNG that ends inside the default image's IDAT data (no fcTL, no IEND)
    // has no further frame, whatever acTL says.
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/030.png";
    let data = open_and_read(path);
    let first_idat = data.windows(4).position(|w| w == b"IDAT").unwrap() + 4;
    let truncated = &data[..first_idat + 100];

    let mut decoder = PngDecoder::new(ZCursor::new(truncated));
    decoder.decode_headers().unwrap();
    let mut iterations = 0;
    while decoder.more_frames() {
        iterations += 1;
        assert!(
            iterations <= 1,
            "more_frames() stays true at the end of the input"
        );
        decoder.decode_headers().unwrap();
        let _ = decoder.decode_raw();
    }
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
    let expected_hash = 1483657996133460445_u64;
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
    let expected_hash = 11531053282805834058_u64;
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
