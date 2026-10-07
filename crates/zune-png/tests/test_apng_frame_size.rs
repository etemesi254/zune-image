/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;

use zune_core::bytestream::ZCursor;
use zune_png::{ApngContext, PngDecoder};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

#[test]
fn fctl_with_zero_width_is_an_error() {
    // APNG specification, constraints on frame regions: width > 0 and height > 0
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/animated_ball.png";
    let mut data = read(path).unwrap();
    let fctl = data.windows(4).position(|w| w == b"fcTL").unwrap();
    // fcTL body: sequence number, then width
    data[fctl + 8..fctl + 12].copy_from_slice(&0_u32.to_be_bytes());
    let crc = crc32(&data[fctl..fctl + 4 + 26]);
    data[fctl + 30..fctl + 34].copy_from_slice(&crc.to_be_bytes());

    let mut decoder = PngDecoder::new(ZCursor::new(&data));
    assert!(decoder.decode_headers().is_err());
}

#[test]
fn process_frame_with_zero_width_does_not_panic() {
    // A zero-width frame made ApngContext::process_frame panic ("chunk size must be
    // non-zero") instead of returning an error.
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/animated_ball.png";
    let data = read(path).unwrap();
    let mut decoder = PngDecoder::new(ZCursor::new(&data));
    decoder.decode_headers().unwrap();
    let info = decoder.info().unwrap().clone();
    let colorspace = decoder.colorspace().unwrap();
    let mut context = ApngContext::<u8>::new(&info, colorspace);
    let mut output = vec![0; info.width * info.height * colorspace.num_components()];

    let mut frame = decoder.frame_info().unwrap();
    frame.width = 0;
    assert!(context.process_frame(&frame, &[], &mut output).is_err());
}
