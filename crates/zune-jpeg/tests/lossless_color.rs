use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

const HUFFMAN: &[u8] =
    include_bytes!("../../../test-images/jpeg/lossless/32x32x8_ycbcr_interleaved.jpg");
const COMPONENTS: &[u8] =
    include_bytes!("../../../test-images/jpeg/lossless/32x32x8_ycbcr_interleaved.yuv");
const RGB: &[u8] =
    include_bytes!("../../../test-images/jpeg/lossless/32x32x8_ycbcr_interleaved.rgb");

fn check(data: &[u8], color: ColorSpace, expected: &[u8], vector: bool) {
    check_image(data, color, expected, vector, (32, 32));
}

fn check_image(
    data: &[u8], color: ColorSpace, expected: &[u8], vector: bool, dimensions: (usize, usize)
) {
    let (width, height) = dimensions;
    let options = DecoderOptions::default()
        .set_use_unsafe(vector)
        .set_strict_mode(true)
        .jpeg_set_out_colorspace(color);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    assert_eq!(decoder.dimensions(), Some(dimensions));
    assert_eq!(decoder.info().unwrap().pixel_density, 8);
    assert_eq!(decoder.output_colorspace(), Some(color));
    assert_eq!(decoder.output_buffer_size(), Some(expected.len()));
    assert_eq!(decoder.decode().unwrap(), expected);
    let stride = width * color.num_components();
    assert_eq!(expected.len(), height * stride);
    let mut output = vec![0xa5; expected.len() + 20];
    decoder.decode_into(&mut output[7..]).unwrap();
    assert_eq!(&output[7..7 + expected.len()], expected);
    assert!(output[..7]
        .iter()
        .chain(&output[7 + expected.len()..])
        .all(|&b| b == 0xa5));
    assert_eq!(decoder.decoded_scanlines(), Some(height));
}

#[test]
fn public_huffman_interleaved_components_are_exact() {
    check(HUFFMAN, ColorSpace::YCbCr, COMPONENTS, false);
}

#[test]
fn public_huffman_rgb_matches_explicit_conversion_contract() {
    for vector in [false, true] {
        check(HUFFMAN, ColorSpace::RGB, RGB, vector);
    }
}

#[cfg(feature = "arith")]
#[test]
fn public_arithmetic_grayscale_is_exact() {
    check(
        include_bytes!("../../../test-images/jpeg/lossless/arith/32x32x8_grayscale.jpg"),
        ColorSpace::Luma,
        include_bytes!("../../../test-images/jpeg/lossless/32x32x8_grayscale.gray"),
        false
    );
}

#[cfg(feature = "arith")]
#[test]
fn public_arithmetic_interleaved_components_and_rgb_are_exact() {
    let data =
        include_bytes!("../../../test-images/jpeg/lossless/arith/32x32x8_ycbcr_interleaved.jpg");
    check(data, ColorSpace::YCbCr, COMPONENTS, false);
    for vector in [false, true] {
        check(data, ColorSpace::RGB, RGB, vector);
    }
}

macro_rules! generated {
    ($name:literal) => {{
        let data: &[u8] = include_bytes!(concat!(
            "../../../test-images/jpeg/lossless/generated/",
            $name
        ));
        data
    }};
}

#[test]
fn noninterleaved_lossless_scans_are_explicitly_unsupported() {
    use zune_jpeg::errors::DecodeErrors;
    for data in [
        include_bytes!("../../../test-images/jpeg/lossless/32x32x8_ycbcr.jpg").as_slice(),
        include_bytes!("../../../test-images/jpeg/lossless/arith/32x32x8_ycbcr.jpg").as_slice()
    ] {
        let mut decoder = JpegDecoder::new(ZCursor::new(data));
        decoder.decode_headers().unwrap();
        assert_eq!(decoder.dimensions(), Some((32, 32)));
        let mut output = [0xa5; 3072];
        assert!(matches!(
            decoder.decode_into(&mut output),
            Err(DecodeErrors::Unsupported(_))
        ));
        assert_eq!(output, [0xa5; 3072]);
    }
}

#[cfg(feature = "arith")]
#[test]
fn arithmetic_magnitude_overflow_is_bounded_and_marker_fill_is_valid() {
    let data = generated!("arith-residual-overflow.jpg");
    let error = JpegDecoder::new(ZCursor::new(data)).decode().unwrap_err();
    assert!(error.to_string().contains("overflow"), "{error:?}");
    let mut data = generated!("arith-3-restart.jpg").to_vec();
    let restart = data
        .windows(2)
        .position(|bytes| bytes == [255, 0xd0])
        .unwrap();
    data.insert(restart, 255);
    let eoi = data.len() - 2;
    data.insert(eoi, 255);
    check_source(&data, generated!("source-33x20-3.raw"), (33, 20), 0, 3);
}

fn rgb_contract(components: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    for pixel in components.chunks_exact(3) {
        let base = i32::from(pixel[0]) * 16384 + 8191;
        let cb = i32::from(pixel[1]) - 128;
        let cr = i32::from(pixel[2]) - 128;
        output.extend([
            ((base + 22970 * cr) >> 14).clamp(0, 255) as u8,
            ((base - 11700 * cr - 5638 * cb) >> 14).clamp(0, 255) as u8,
            ((base + 29032 * cb) >> 14).clamp(0, 255) as u8
        ]);
    }
    output
}

fn check_source(data: &[u8], source: &[u8], dimensions: (usize, usize), pt: u8, count: usize) {
    let expected: Vec<_> = source.iter().map(|&sample| (sample >> pt) << pt).collect();
    let color = if count == 1 { ColorSpace::Luma } else { ColorSpace::YCbCr };
    check_image(data, color, &expected, false, dimensions);
    if count == 3 {
        let rgb = rgb_contract(&expected);
        for vector in [false, true] {
            check_image(data, ColorSpace::RGB, &rgb, vector, dimensions);
        }
    }
}

#[test]
fn huffman_component_order_tables_predictors_and_point_transforms() {
    let cases = [
        generated!("huffman-3-p1.jpg"),
        generated!("huffman-3-p2.jpg"),
        generated!("huffman-3-p3.jpg"),
        generated!("huffman-3-p4.jpg"),
        generated!("huffman-3-p5.jpg"),
        generated!("huffman-3-p6.jpg"),
        generated!("huffman-3-p7.jpg")
    ];
    for (pt, data) in cases.into_iter().enumerate() {
        check_source(data, generated!("source-5x3-3.raw"), (5, 3), pt as u8, 3);
    }
    check_source(
        generated!("huffman-3-restart.jpg"),
        generated!("source-33x20-3.raw"),
        (33, 20),
        0,
        3
    );
    check_source(
        generated!("huffman-3-narrow.jpg"),
        generated!("source-1x9-3.raw"),
        (1, 9),
        7,
        3
    );
    check_source(
        generated!("huffman-3-single.jpg"),
        generated!("source-1x1-3.raw"),
        (1, 1),
        0,
        3
    );
}

#[cfg(feature = "arith")]
#[test]
fn arithmetic_lossless_contexts_predictors_conditioning_and_restarts() {
    let gray = [
        generated!("arith-1-p1.jpg"),
        generated!("arith-1-p2.jpg"),
        generated!("arith-1-p3.jpg"),
        generated!("arith-1-p4.jpg"),
        generated!("arith-1-p5.jpg"),
        generated!("arith-1-p6.jpg"),
        generated!("arith-1-p7.jpg")
    ];
    let color = [
        generated!("arith-3-p1.jpg"),
        generated!("arith-3-p2.jpg"),
        generated!("arith-3-p3.jpg"),
        generated!("arith-3-p4.jpg"),
        generated!("arith-3-p5.jpg"),
        generated!("arith-3-p6.jpg"),
        generated!("arith-3-p7.jpg")
    ];
    for (pt, (gray, color)) in gray.into_iter().zip(color).enumerate() {
        check_source(gray, generated!("source-5x3-1.raw"), (5, 3), pt as u8, 1);
        check_source(color, generated!("source-5x3-3.raw"), (5, 3), pt as u8, 3);
    }
    for (data, source, dimensions, pt, count) in [
        (
            generated!("arith-1-restart.jpg"),
            generated!("source-33x20-1.raw"),
            (33, 20),
            0,
            1
        ),
        (
            generated!("arith-3-restart.jpg"),
            generated!("source-33x20-3.raw"),
            (33, 20),
            0,
            3
        ),
        (
            generated!("arith-1-narrow.jpg"),
            generated!("source-1x9-1.raw"),
            (1, 9),
            7,
            1
        ),
        (
            generated!("arith-3-narrow.jpg"),
            generated!("source-1x9-3.raw"),
            (1, 9),
            7,
            3
        ),
        (
            generated!("arith-1-single.jpg"),
            generated!("source-1x1-1.raw"),
            (1, 1),
            0,
            1
        ),
        (
            generated!("arith-3-single.jpg"),
            generated!("source-1x1-3.raw"),
            (1, 1),
            0,
            3
        ),
        (
            generated!("arith-3-dac0-0.jpg"),
            generated!("source-5x3-3.raw"),
            (5, 3),
            0,
            3
        ),
        (
            generated!("arith-3-dac15-15.jpg"),
            generated!("source-5x3-3.raw"),
            (5, 3),
            0,
            3
        )
    ] {
        check_source(data, source, dimensions, pt, count);
    }
}

#[test]
fn packed_output_alpha_luma_and_channel_order_are_preserved() {
    let inputs: &[&[u8]] = &[
        HUFFMAN,
        #[cfg(feature = "arith")]
        include_bytes!("../../../test-images/jpeg/lossless/arith/32x32x8_ycbcr_interleaved.jpg")
    ];
    for &data in inputs {
        for color in [
            ColorSpace::RGBA,
            ColorSpace::BGR,
            ColorSpace::BGRA,
            ColorSpace::Luma,
            ColorSpace::LumaA
        ] {
            let mut expected = Vec::new();
            for (rgb, yuv) in RGB.chunks_exact(3).zip(COMPONENTS.chunks_exact(3)) {
                match color {
                    ColorSpace::Luma => expected.push(yuv[0]),
                    ColorSpace::LumaA => expected.extend([yuv[0], 255]),
                    ColorSpace::RGB | ColorSpace::RGBA => expected.extend(rgb),
                    ColorSpace::BGR | ColorSpace::BGRA => expected.extend([rgb[2], rgb[1], rgb[0]]),
                    _ => unreachable!()
                }
                if matches!(color, ColorSpace::RGBA | ColorSpace::BGRA) {
                    expected.push(255);
                }
            }
            for vector in [false, true] {
                check(data, color, &expected, vector);
            }
        }
    }
}

#[test]
fn unsupported_organizations_tables_and_resource_limits_do_not_write() {
    use zune_jpeg::errors::DecodeErrors;
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::YCbCr);
    let mut short = [0xa5; 3071];
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(HUFFMAN), options);
    assert!(matches!(
        decoder.decode_into(&mut short),
        Err(DecodeErrors::TooSmallOutput(3072, 3071))
    ));
    assert_eq!(short, [0xa5; 3071]);
    for options in [options.set_max_width(31), options.set_max_height(31)] {
        assert!(
            JpegDecoder::new_with_options(ZCursor::new(HUFFMAN), options)
                .decode()
                .is_err()
        );
    }
    let sof = HUFFMAN
        .windows(2)
        .position(|bytes| bytes == [255, 195])
        .unwrap();
    let sos = HUFFMAN
        .windows(2)
        .position(|bytes| bytes == [255, 218])
        .unwrap();
    for (at, value) in [
        (sof + 11, 0x22),
        (sof + 4, 12),
        (sos + 6, 0x30),
        (sos + 6, 0x40)
    ] {
        let mut data = HUFFMAN.to_vec();
        data[at] = value;
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
        let mut output = [0xa5; 3072];
        assert!(decoder.decode_into(&mut output).is_err());
        assert_eq!(output, [0xa5; 3072]);
    }
}

#[cfg(feature = "arith")]
#[test]
fn arithmetic_invalid_conditioning_markers_and_truncation_are_errors() {
    let source = generated!("arith-3-p1.jpg");
    let dac = source
        .windows(2)
        .position(|bytes| bytes == [255, 204])
        .unwrap();
    let sos = source
        .windows(2)
        .position(|bytes| bytes == [255, 218])
        .unwrap();
    for (at, value) in [
        (dac + 4, 4),
        (dac + 4, 0x10),
        (dac + 5, 0x12),
        (sos + 6, 0x40)
    ] {
        let mut data = source.to_vec();
        data[at] = value;
        let mut decoder = JpegDecoder::new(ZCursor::new(data));
        let mut output = [0xa5; 45];
        assert!(decoder.decode_into(&mut output).is_err());
        assert_eq!(output, [0xa5; 45]);
    }
    for source in [
        generated!("arith-1-single.jpg"),
        generated!("arith-3-p1.jpg"),
        generated!("arith-3-restart.jpg")
    ] {
        for cutoff in 0..source.len() {
            let options = DecoderOptions::default().set_strict_mode(true);
            assert!(
                JpegDecoder::new_with_options(ZCursor::new(&source[..cutoff]), options)
                    .decode()
                    .is_err(),
                "cutoff {cutoff} of {}",
                source.len()
            );
        }
    }
    let mut bad = generated!("arith-3-restart.jpg").to_vec();
    let rst = bad
        .windows(2)
        .position(|bytes| bytes == [255, 0xd0])
        .unwrap();
    bad[rst + 1] = 0xd4;
    assert!(JpegDecoder::new(ZCursor::new(&bad)).decode().is_err());
    let mut bad = source.to_vec();
    let entropy = sos + 2 + usize::from(u16::from_be_bytes([bad[sos + 2], bad[sos + 3]]));
    bad.splice(entropy..entropy, [255, 254]);
    assert!(JpegDecoder::new(ZCursor::new(&bad)).decode().is_err());
}

#[cfg(not(feature = "arith"))]
#[test]
fn lossless_arithmetic_does_not_activate_the_arithmetic_feature() {
    use zune_jpeg::errors::{DecodeErrors, UnsupportedSchemes};
    let data = include_bytes!("../../../test-images/jpeg/lossless/arith/32x32x8_grayscale.jpg");
    assert!(matches!(
        JpegDecoder::new(ZCursor::new(data)).decode(),
        Err(DecodeErrors::Unsupported(
            UnsupportedSchemes::LosslessArithmetic
        ))
    ));
}
