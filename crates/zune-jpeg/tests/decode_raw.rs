/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for `JpegDecoder::raw_output()`.
//!
//! Mirrors libjpeg-turbo's `jpeg_read_raw_data` semantics: each component's
//! post-IDCT samples are written directly into a caller-provided plane,
//! skipping upsampling and color conversion.

use zune_core::bytestream::ZCursor;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::JpegDecoder;

fn with_component_ids(data: &[u8], ids: &[u8]) -> Vec<u8> {
    let mut result = data.to_vec();
    let sof = result
        .windows(2)
        .position(|bytes| bytes[0] == 0xff && (0xc0..=0xc2).contains(&bytes[1]))
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
        .filter_map(|(offset, bytes)| (bytes == [0xff, 0xda]).then_some(offset))
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

/// Allocate per-plane buffers sized to the layout reported by
/// `RawDecodeSession::layout()`, then decode and return the populated planes.
fn decode_raw_into_owned(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().expect("decode_headers");
    let mut raw = decoder.raw_output();
    let n = raw.num_components().expect("num_components");
    let layout = raw.layout().expect("raw layout");

    let mut planes: Vec<Vec<u8>> = (0..n).map(|i| vec![0u8; layout[i].byte_size]).collect();
    {
        let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
        raw.decode_into(&mut refs).expect("raw decode");
    }
    planes
}

#[test]
fn decode_raw_rejects_wrong_plane_count() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    // 3-component image, pass 2 planes.
    let mut p0 = vec![0u8; layout[0].byte_size];
    let mut p1 = vec![0u8; layout[1].byte_size];
    let mut refs: [&mut [u8]; 2] = [&mut p0, &mut p1];
    let err = raw.decode_into(&mut refs).unwrap_err();
    match err {
        DecodeErrors::Format(_) => {}
        other => panic!("expected Format error, got {other:?}")
    }
}

#[test]
fn decode_raw_rejects_too_small_plane() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let mut p0 = vec![0u8; layout[0].byte_size - 1]; // one byte short
    let mut p1 = vec![0u8; layout[1].byte_size];
    let mut p2 = vec![0u8; layout[2].byte_size];
    let mut refs: [&mut [u8]; 3] = [&mut p0, &mut p1, &mut p2];
    let err = raw.decode_into(&mut refs).unwrap_err();
    match err {
        DecodeErrors::TooSmallOutput(need, got) => {
            assert_eq!(need, layout[0].byte_size);
            assert_eq!(got, layout[0].byte_size - 1);
        }
        other => panic!("expected TooSmallOutput, got {other:?}")
    }
}

#[test]
fn decode_raw_rejects_more_than_four_sof_components() {
    const FIVE_COMPONENTS: &[u8] = &[
        0xff, 0xd8, // SOI
        0xff, 0xc0, // SOF0
        0x00, 0x17, // length = 8 + 3 * 5
        0x08, // precision
        0x00, 0x01, // height
        0x00, 0x01, // width
        0x05, // components
        1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0, 4, 0x11, 0, 5, 0x11, 0
    ];

    let mut decoder = JpegDecoder::new(ZCursor::new(FIVE_COMPONENTS));
    let err = decoder.decode_headers().unwrap_err();
    match err {
        DecodeErrors::SofError(_) => {}
        other => panic!("expected SofError, got {other:?}")
    }
}

#[test]
fn decode_raw_interleaved_420_full_range() {
    // 388x477 4:2:0 photographic JPEG, all 3 components in a single SOS.
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let planes = decode_raw_into_owned(bytes);
    assert_eq!(planes.len(), 3);

    // Photographic content: each plane should span a wide range and the
    // chroma planes should be centred near 128 (neutral).
    let y_min = *planes[0].iter().min().unwrap();
    let y_max = *planes[0].iter().max().unwrap();
    assert!(
        y_max - y_min > 100,
        "Y plane range too narrow: {y_min}..{y_max}"
    );
    for (idx, plane) in planes[1..].iter().enumerate() {
        let pmin = *plane.iter().min().unwrap();
        let pmax = *plane.iter().max().unwrap();
        assert!(pmax > pmin, "chroma plane {} appears constant", idx + 1);
        let avg: u64 = plane.iter().map(|&v| v as u64).sum::<u64>() / plane.len() as u64;
        assert!(
            (96..=160).contains(&avg),
            "chroma plane {} avg {} not in expected near-neutral band",
            idx + 1,
            avg
        );
    }
}

#[test]
fn decode_raw_interleaved_420_deterministic() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let a = decode_raw_into_owned(bytes);
    let b = decode_raw_into_owned(bytes);
    assert_eq!(a, b);
}

#[test]
fn decode_raw_interleaved_no_subsampling() {
    // 4:4:4 image where every plane has the same dimensions.
    let bytes = include_bytes!("../../../test-images/jpeg/medium_no_samp_2500x1786.jpg");
    let planes = decode_raw_into_owned(bytes);
    assert_eq!(planes.len(), 3);
    assert_eq!(planes[0].len(), planes[1].len());
    assert_eq!(planes[1].len(), planes[2].len());
    for (i, plane) in planes.iter().enumerate() {
        let pmin = *plane.iter().min().unwrap();
        let pmax = *plane.iter().max().unwrap();
        assert!(pmax > pmin, "plane {i} appears constant");
    }
}

#[test]
fn decode_raw_grayscale_progressive_single_plane() {
    let bytes = include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let n = raw.num_components().unwrap();
    assert_eq!(n, 1, "expected grayscale (1 component)");
    let layout = raw.layout().unwrap();
    let mut p = vec![0u8; layout[0].byte_size];
    {
        let mut refs: [&mut [u8]; 1] = [&mut p];
        raw.decode_into(&mut refs).expect("raw progressive decode");
    }
    let min = *p.iter().min().unwrap();
    let max = *p.iter().max().unwrap();
    assert!(max > min, "decoded grayscale plane is a single constant");
}

#[test]
fn decode_raw_progressive_four_component() {
    let bytes =
        include_bytes!("../../../test-images/jpeg/Kiara_limited_progressive_four_components.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let n = raw.num_components().unwrap();
    assert_eq!(n, 4, "expected 4-component CMYK/YCCK image");
    let layout = raw.layout().unwrap();
    let mut planes: Vec<Vec<u8>> = (0..n).map(|i| vec![0u8; layout[i].byte_size]).collect();
    {
        let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
        raw.decode_into(&mut refs).expect("raw decode");
    }
    for (i, plane) in planes.iter().enumerate() {
        assert_eq!(plane.len(), layout[i].byte_size);
        let pmin = *plane.iter().min().unwrap();
        let pmax = *plane.iter().max().unwrap();
        assert!(pmax > pmin, "plane {i} appears constant");
    }
}

#[test]
fn decode_raw_works_after_explicit_decode_headers() {
    let bytes = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let n = raw.num_components().unwrap();
    let mut planes: Vec<Vec<u8>> = (0..n).map(|i| vec![0u8; layout[i].byte_size]).collect();
    let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into(&mut refs).unwrap();
}

#[test]
fn decode_raw_ignores_out_colorspace_setting() {
    // Regression test: previously, configuring `out_colorspace = Luma` on a
    // 3-component YCbCr image caused the baseline decoder to skip allocating
    // raw_coeff for Cb / Cr (their `comp.needed` was set to false), so
    // `decode_raw` would either panic or return garbage planes.
    //
    // In raw mode we want every component regardless of the configured
    // output colorspace.
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::DecoderOptions;

    let bytes = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), opts);
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let n = raw.num_components().unwrap();
    assert_eq!(n, 3, "fixture is 3-component");
    let layout = raw.layout().unwrap();
    let mut planes: Vec<Vec<u8>> = (0..n).map(|i| vec![0u8; layout[i].byte_size]).collect();
    {
        let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
        raw.decode_into(&mut refs)
            .expect("decode_raw should succeed regardless of out_colorspace");
    }
    // All three planes must contain real decoded data, not zeros.
    for (i, plane) in planes.iter().enumerate() {
        let pmin = *plane.iter().min().unwrap();
        let pmax = *plane.iter().max().unwrap();
        assert!(
            pmax > pmin,
            "plane {i} appears constant ({pmin}); raw mode must decode all components"
        );
    }
}

#[test]
fn decode_raw_unaligned_dimensions_decode_succeeds() {
    // 605x806 image with unaligned dimensions: width padded 605 -> 608,
    // height padded 806 -> 808. Exercises the per-row padding-truncation
    // path in `copy_raw_planes_for_mcu_stripe` and the trailing-padding-row
    // bookkeeping.
    let bytes = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    assert_eq!(layout[0].width, 605);
    assert_eq!(layout[0].height, 806);
    assert_eq!(layout[0].stride, 608);
    assert_eq!(layout[0].allocated_height, 808);

    let n = raw.num_components().unwrap();
    let mut planes: Vec<Vec<u8>> = (0..n).map(|i| vec![0u8; layout[i].byte_size]).collect();
    {
        let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
        raw.decode_into(&mut refs).unwrap();
    }
    // Spot-check: meaningful Y region (rows 0..806, cols 0..605) has data.
    let y = &planes[0];
    let mut has_non_zero = false;
    for row in 0..layout[0].height {
        let v = y[row * layout[0].stride];
        if v != 0 {
            has_non_zero = true;
            break;
        }
    }
    assert!(has_non_zero, "first column of Y plane is all zero");
}

/// Manual nearest-neighbour upsampling of `src` (size `sw x sh`, row stride
/// `s_stride`) into a `tw x th` plane.
fn nearest_upsample(
    src: &[u8], sw: usize, sh: usize, s_stride: usize, tw: usize, th: usize
) -> Vec<u8> {
    let mut out = vec![0u8; tw * th];
    for y in 0..th {
        let sy = (y * sh) / th;
        for x in 0..tw {
            let sx = (x * sw) / tw;
            out[y * tw + x] = src[sy * s_stride + sx];
        }
    }
    out
}

/// JFIF YCbCr -> RGB conversion (8-bit).
fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    let yf = y as f32;
    let cbf = cb as f32 - 128.0;
    let crf = cr as f32 - 128.0;
    let r = yf + 1.402 * crf;
    let g = yf - 0.344136 * cbf - 0.714136 * crf;
    let b = yf + 1.772 * cbf;
    [
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8
    ]
}

fn assert_raw_content_matches_decode_within_upsample_tolerance(
    name: &str, bytes: &[u8], max_mean_delta: u64
) {
    let rgb = {
        let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
        decoder.decode().unwrap()
    };

    let mut raw_decoder = JpegDecoder::new(ZCursor::new(bytes));
    raw_decoder.decode_headers().unwrap();
    let (w, h) = raw_decoder.dimensions().unwrap();
    let mut raw = raw_decoder.raw_output();
    let layout = raw.layout().unwrap();
    let n = raw.num_components().unwrap();
    assert_eq!(n, 3, "{name}: expected YCbCr fixture");
    let mut planes: Vec<Vec<u8>> = (0..n).map(|i| vec![0u8; layout[i].byte_size]).collect();
    {
        let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
        raw.decode_into(&mut refs).unwrap();
    }

    let y_up = nearest_upsample(
        &planes[0],
        layout[0].width,
        layout[0].height,
        layout[0].stride,
        w,
        h
    );
    let cb_up = nearest_upsample(
        &planes[1],
        layout[1].width,
        layout[1].height,
        layout[1].stride,
        w,
        h
    );
    let cr_up = nearest_upsample(
        &planes[2],
        layout[2].width,
        layout[2].height,
        layout[2].stride,
        w,
        h
    );

    let mut sum_dr = 0u64;
    let mut sum_dg = 0u64;
    let mut sum_db = 0u64;
    let total = (w * h) as u64;
    for i in 0..(w * h) {
        let [r, g, b] = ycbcr_to_rgb(y_up[i], cb_up[i], cr_up[i]);
        let rr = rgb[i * 3];
        let gg = rgb[i * 3 + 1];
        let bb = rgb[i * 3 + 2];
        sum_dr += (r as i32 - rr as i32).unsigned_abs() as u64;
        sum_dg += (g as i32 - gg as i32).unsigned_abs() as u64;
        sum_db += (b as i32 - bb as i32).unsigned_abs() as u64;
    }
    let avg_dr = sum_dr / total;
    let avg_dg = sum_dg / total;
    let avg_db = sum_db / total;
    assert!(
        avg_dr < max_mean_delta && avg_dg < max_mean_delta && avg_db < max_mean_delta,
        "{name}: mean abs RGB delta too large: dr={avg_dr} dg={avg_dg} db={avg_db}"
    );
}

#[test]
fn decode_raw_content_matches_decode_within_upsample_tolerance() {
    // Decode the same image two ways and assert the planes, after
    // nearest-neighbour upsampling and JFIF YCbCr -> RGB, are statistically
    // close to the regular RGB output. This guards against plane-ordering,
    // stride, and clamp-cast regressions.
    //
    // Tolerance is loose because zune-jpeg uses a fancier upsampler than the
    // nearest-neighbour we apply in test code; we only assert that the mean
    // absolute difference per channel is small.
    // A plane swap, stride bug, or wrong YCbCr->RGB matrix would produce
    // mean absolute deltas in the dozens; nearest-vs-fancy-upsample alone
    // typically produces single-digit deltas on photographic content.
    assert_raw_content_matches_decode_within_upsample_tolerance(
        "interleaved_420",
        include_bytes!("../../../test-images/jpeg/2029.jpg"),
        12
    );
}

#[test]
fn decode_raw_non_interleaved_content_matches_decode() {
    for (name, bytes) in [
        (
            "non_interleaved_444",
            &include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg")[..]
        ),
        (
            "non_interleaved_420",
            &include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg")[..]
        ),
        (
            "non_interleaved_422",
            &include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg")[..]
        ),
        (
            "non_interleaved_440",
            &include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg")[..]
        )
    ] {
        assert_raw_content_matches_decode_within_upsample_tolerance(name, bytes, 24);
    }
}

// decode_raw_strided: Skia-style logically-sized planes.

#[test]
fn decode_raw_strided_logical_size_matches_padded_decode() {
    // 605x806 4:2:0 image: padded layout is 608x808 / 304x404; logical layout
    // is 605x806 / 303x403. Verify that decode_raw_strided into a logical
    // (width-stride) buffer returns identical samples within
    // [0,width) x [0,height) versus decode_raw into the padded buffer.
    let bytes = include_bytes!("../../../test-images/jpeg/fox410.jpg");

    // Padded reference.
    let padded = decode_raw_into_owned(bytes);

    // Logical-sized via decode_raw_strided.
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let n = raw.num_components().unwrap();

    let strides: Vec<usize> = (0..n).map(|i| layout[i].width).collect();
    let mut logical: Vec<Vec<u8>> = (0..n)
        .map(|i| vec![0u8; strides[i] * layout[i].height])
        .collect();
    {
        let mut refs: Vec<&mut [u8]> = logical.iter_mut().map(Vec::as_mut_slice).collect();
        raw.decode_into_strided(&mut refs, &strides).unwrap();
    }

    for i in 0..n {
        let pi = layout[i];
        for y in 0..pi.height {
            for x in 0..pi.width {
                let logical_byte = logical[i][y * strides[i] + x];
                let padded_byte = padded[i][y * pi.stride + x];
                assert_eq!(
                    logical_byte, padded_byte,
                    "mismatch at component {i} ({},{})",
                    x, y
                );
            }
        }
        assert_eq!(logical[i].len(), strides[i] * pi.height);
    }
}

#[test]
fn decode_raw_strided_accepts_oversized_stride() {
    // Caller supplies a larger-than-width stride (e.g. row alignment of 64).
    // Verify the per-row padding bytes stay zero and the meaningful samples
    // match the logical decode.
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let padded = decode_raw_into_owned(bytes);

    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let n = raw.num_components().unwrap();

    // Round each width up to the next multiple of 64.
    let strides: Vec<usize> = (0..n).map(|i| (layout[i].width + 63) & !63).collect();
    let mut planes: Vec<Vec<u8>> = (0..n)
        .map(|i| vec![0xAAu8; strides[i] * layout[i].height])
        .collect();
    {
        let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
        raw.decode_into_strided(&mut refs, &strides).unwrap();
    }

    for i in 0..n {
        let pi = layout[i];
        for y in 0..pi.height {
            for x in 0..pi.width {
                assert_eq!(
                    planes[i][y * strides[i] + x],
                    padded[i][y * pi.stride + x],
                    "mismatch at component {i} ({},{})",
                    x,
                    y
                );
            }
            // Trailing pad bytes from logical width up to caller stride
            // must remain at the sentinel value (we did not write them).
            for x in pi.width..strides[i] {
                assert_eq!(
                    planes[i][y * strides[i] + x],
                    0xAA,
                    "stride padding modified at component {i} ({},{})",
                    x,
                    y
                );
            }
        }
    }
}

#[test]
fn decode_raw_strided_rejects_too_small_stride() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let n = raw.num_components().unwrap();

    // Stride for component 0 is one less than the logical width.
    let strides: Vec<usize> = (0..n)
        .map(|i| if i == 0 { layout[i].width - 1 } else { layout[i].width })
        .collect();
    let mut planes: Vec<Vec<u8>> = (0..n)
        .map(|i| vec![0u8; strides[i].max(1) * layout[i].height])
        .collect();
    let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    let err = raw.decode_into_strided(&mut refs, &strides).unwrap_err();
    match err {
        DecodeErrors::Format(_) => {}
        other => panic!("expected Format error, got {other:?}")
    }
}

#[test]
fn decode_raw_strided_rejects_too_small_buffer() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let n = raw.num_components().unwrap();

    let strides: Vec<usize> = (0..n).map(|i| layout[i].width).collect();
    let mut planes: Vec<Vec<u8>> = (0..n)
        .map(|i| vec![0u8; strides[i] * layout[i].height])
        .collect();
    // Truncate component 0 by one byte.
    planes[0].pop();
    let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    let err = raw.decode_into_strided(&mut refs, &strides).unwrap_err();
    match err {
        DecodeErrors::TooSmallOutput(_, _) => {}
        other => panic!("expected TooSmallOutput, got {other:?}")
    }
}

#[test]
fn decode_raw_strided_unaligned_dimensions_no_pad_writes() {
    // Logical-sized buffer on a non-DCT-aligned image. Verify the trailing
    // row past `height` is never touched by the decoder (no scratch leak).
    let bytes = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let n = raw.num_components().unwrap();

    // Allocate exactly one extra sentinel row past `height` in the same
    // backing Vec, but tell the decoder the buffer is `stride * height`.
    let strides: Vec<usize> = (0..n).map(|i| layout[i].width).collect();
    let buf_lens: Vec<usize> = (0..n).map(|i| strides[i] * layout[i].height).collect();
    let mut backing: Vec<Vec<u8>> = (0..n)
        .map(|i| vec![0xCDu8; buf_lens[i] + strides[i]]) // one extra row of sentinel
        .collect();

    {
        // Slice each plane to exactly buf_lens[i] so the decoder cannot
        // address past it.
        let mut refs: Vec<&mut [u8]> = backing
            .iter_mut()
            .enumerate()
            .map(|(i, v)| &mut v[..buf_lens[i]])
            .collect();
        raw.decode_into_strided(&mut refs, &strides).unwrap();
    }

    for i in 0..n {
        let extra = &backing[i][buf_lens[i]..];
        assert!(
            extra.iter().all(|&b| b == 0xCD),
            "trailing sentinel row clobbered for component {i}"
        );
    }
}

// component_ids() metadata.

#[test]
fn component_ids_return_canonical_sof_selectors() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let raw = decoder.raw_output();
    let ids = raw.component_ids().expect("component_ids after headers");
    assert_eq!(ids, vec![1, 2, 3]);
}

#[test]
fn component_ids_preserve_noncanonical_sof_selectors() {
    let bytes = with_component_ids(include_bytes!("../../../test-images/jpeg/2029.jpg"), b"RGB");
    let mut decoder = JpegDecoder::new(ZCursor::new(&bytes));
    decoder.decode_headers().unwrap();
    let raw = decoder.raw_output();
    assert_eq!(raw.component_ids().as_deref(), Some(b"RGB".as_slice()));
}

#[test]
fn component_ids_returns_none_before_headers() {
    let bytes = include_bytes!("../../../test-images/jpeg/2029.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    let raw = decoder.raw_output();
    assert!(raw.component_ids().is_none());
}

#[test]
fn dnl_whole_raw_apis_are_rejected_before_false_completion() {
    let bytes = include_bytes!("images/dnl_image.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let mut planes = [];

    assert!(matches!(
        raw.decode_into(&mut planes),
        Err(DecodeErrors::FormatStatic(
            "raw output does not support DNL images"
        ))
    ));
    assert!(matches!(
        raw.decode_into_strided(&mut planes, &[]),
        Err(DecodeErrors::FormatStatic(
            "raw output does not support DNL images"
        ))
    ));
}
