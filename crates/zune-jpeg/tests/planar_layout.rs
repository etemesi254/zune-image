/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for raw output session layout and component metadata.
//!
//! These verify the per-component plane geometry exposed for raw planar
//! output, against fixture images with known dimensions and sampling
//! factors. The math mirrors libjpeg-turbo's `jpeg_read_raw_data`:
//! component dimensions are derived from sampling factors, then both
//! axes are rounded up to DCT-block (8 sample) boundaries.

use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

fn layout_for(bytes: &[u8]) -> ([zune_jpeg::PlaneInfo; 4], usize) {
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().expect("decode_headers failed");
    let raw = decoder.raw_output();
    let n = raw.num_components().expect("num_components");
    let layout = raw.layout().expect("raw layout");
    (layout, n)
}

fn assert_sampling_factors(layout: &[zune_jpeg::PlaneInfo], expected: &[(usize, usize)]) {
    let actual: Vec<(usize, usize)> = layout
        .iter()
        .map(|plane| {
            (
                plane.horizontal_sampling_factor,
                plane.vertical_sampling_factor
            )
        })
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn planar_layout_unavailable_before_headers() {
    let mut decoder = JpegDecoder::new(ZCursor::new(&[][..]));
    let raw = decoder.raw_output();
    assert!(raw.num_components().is_none());
    assert!(raw.layout().is_none());
}

#[test]
fn planar_layout_444_64x64_aligned() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let (layout, n) = layout_for(bytes);
    assert_eq!(n, 3, "expected 3 components for YCbCr 4:4:4");
    assert_sampling_factors(&layout[..n], &[(1, 1), (1, 1), (1, 1)]);
    for plane in &layout[..3] {
        assert_eq!(plane.width, 64);
        assert_eq!(plane.height, 64);
        assert_eq!(plane.stride, 64);
        assert_eq!(plane.allocated_height, 64);
        assert_eq!(plane.byte_size, 64 * 64);
    }
    // Trailing slot is unused/zero.
    assert_eq!(layout[3], zune_jpeg::PlaneInfo::default());
}

#[test]
fn planar_layout_422_64x64_aligned() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg");
    let (layout, n) = layout_for(bytes);
    assert_eq!(n, 3);
    assert_sampling_factors(&layout[..n], &[(2, 1), (1, 1), (1, 1)]);
    // Y: full resolution.
    assert_eq!(layout[0].width, 64);
    assert_eq!(layout[0].height, 64);
    assert_eq!(layout[0].stride, 64);
    assert_eq!(layout[0].allocated_height, 64);
    // Cb, Cr: half horizontal, full vertical.
    for plane in &layout[1..3] {
        assert_eq!(plane.width, 32);
        assert_eq!(plane.height, 64);
        assert_eq!(plane.stride, 32);
        assert_eq!(plane.allocated_height, 64);
        assert_eq!(plane.byte_size, 32 * 64);
    }
}

#[test]
fn planar_layout_420_64x64_aligned() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let (layout, n) = layout_for(bytes);
    assert_eq!(n, 3);
    assert_sampling_factors(&layout[..n], &[(2, 2), (1, 1), (1, 1)]);
    assert_eq!(layout[0].width, 64);
    assert_eq!(layout[0].height, 64);
    assert_eq!(layout[0].stride, 64);
    assert_eq!(layout[0].allocated_height, 64);
    for plane in &layout[1..3] {
        assert_eq!(plane.width, 32);
        assert_eq!(plane.height, 32);
        assert_eq!(plane.stride, 32);
        assert_eq!(plane.allocated_height, 32);
        assert_eq!(plane.byte_size, 32 * 32);
    }
}

#[test]
fn planar_layout_440_64x64_aligned() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg");
    let (layout, n) = layout_for(bytes);
    assert_eq!(n, 3);
    assert_sampling_factors(&layout[..n], &[(1, 2), (1, 1), (1, 1)]);
    // Y: full resolution.
    assert_eq!(layout[0].width, 64);
    assert_eq!(layout[0].height, 64);
    // Cb, Cr: full horizontal, half vertical.
    for plane in &layout[1..3] {
        assert_eq!(plane.width, 64);
        assert_eq!(plane.height, 32);
        assert_eq!(plane.stride, 64);
        assert_eq!(plane.allocated_height, 32);
    }
}

#[test]
fn planar_layout_422_65x65_unaligned_pads_to_dct_block() {
    let bytes = include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg");
    let (layout, n) = layout_for(bytes);
    assert_eq!(n, 3);
    // Y: 65x65 → padded to 72x72.
    assert_eq!(layout[0].width, 65);
    assert_eq!(layout[0].height, 65);
    assert_eq!(layout[0].stride, 72);
    assert_eq!(layout[0].allocated_height, 72);
    assert_eq!(layout[0].byte_size, 72 * 72);
    // Cb, Cr: ceil(65/2) = 33 wide, 65 tall → padded to 40x72.
    for plane in &layout[1..3] {
        assert_eq!(plane.width, 33);
        assert_eq!(plane.height, 65);
        assert_eq!(plane.stride, 40);
        assert_eq!(plane.allocated_height, 72);
        assert_eq!(plane.byte_size, 40 * 72);
    }
}

#[test]
fn planar_layout_exposes_411_sampling_factors() {
    let mut bytes =
        include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg").to_vec();
    let sof = bytes
        .windows(2)
        .position(|marker| marker == [0xff, 0xc0])
        .expect("SOF0 marker");
    bytes[sof + 11] = 0x41;

    let (layout, n) = layout_for(&bytes);
    assert_eq!(n, 3);
    assert_sampling_factors(&layout[..n], &[(4, 1), (1, 1), (1, 1)]);
}

#[test]
fn planar_layout_exposes_410_sampling_factors() {
    let bytes = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let (layout, n) = layout_for(bytes);
    assert_eq!(n, 3);
    assert_sampling_factors(&layout[..n], &[(4, 2), (1, 1), (1, 1)]);
}
