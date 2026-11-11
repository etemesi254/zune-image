//! Native box reconstruction when visible chroma width is one or two samples.
//! Synthetic fixtures cover partial MCUs and multiple stripes. Expected pixels
//! were frozen using libjpeg-turbo 2.1.5 with accurate IDCT.

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::{JpegDecoder, ScanlineReadStatus, ScanlineStatus};

const CASES: [(&[u8], usize, usize, &[u8]); 3] = [
    (
        include_bytes!("../../../test-images/jpeg/fancy_narrow_422_4x3.jpg"),
        4,
        3,
        &[
            32, 32, 32, 255, 255, 255, 33, 32, 37, 255, 254, 255, 32, 32, 32, 255, 255, 255, 33,
            32, 37, 255, 254, 255, 32, 32, 32, 255, 255, 255, 33, 32, 37, 255, 254, 255
        ]
    ),
    (
        include_bytes!("../../../test-images/jpeg/fancy_narrow_420_2x3.jpg"),
        2,
        3,
        &[
            0, 120, 34, 0, 167, 81, 49, 221, 135, 51, 223, 137, 202, 146, 87, 149, 93, 34
        ]
    ),
    (
        include_bytes!("../../../test-images/jpeg/fancy_narrow_420_4x67.jpg"),
        4,
        67,
        include_bytes!("../../../test-images/jpeg/fancy_narrow_420_4x67.rgb")
    )
];

fn packed(rgb: &[u8], colorspace: ColorSpace) -> Vec<u8> {
    let mut output = Vec::new();
    for pixel in rgb.chunks_exact(3) {
        if matches!(colorspace, ColorSpace::BGR | ColorSpace::BGRA) {
            output.extend([pixel[2], pixel[1], pixel[0]]);
        } else {
            output.extend_from_slice(pixel);
        }
        if colorspace.num_components() == 4 {
            output.push(255);
        }
    }
    output
}

fn with_dnl(jpeg: &[u8], height: usize) -> Vec<u8> {
    let mut output = jpeg.to_vec();
    let sof = output
        .windows(2)
        .position(|bytes| bytes == [0xff, 0xc0])
        .unwrap();
    assert_eq!(
        usize::from(u16::from_be_bytes([output[sof + 5], output[sof + 6]])),
        height
    );
    output[sof + 5..sof + 7].fill(0);
    assert!(output.ends_with(&[0xff, 0xd9]));
    output.truncate(output.len() - 2);
    output.extend_from_slice(&[0xff, 0xdc, 0, 4]);
    output.extend_from_slice(&u16::try_from(height).unwrap().to_be_bytes());
    output.extend_from_slice(&[0xff, 0xd9]);
    output
}

// RGB expectations are from libjpeg-turbo 2.1.5, accurate IDCT, default upsampling.
#[test]
fn narrow_chroma_matches_native_box_reconstruction() {
    for (jpeg, width, height, rgb) in CASES {
        for use_simd in [false, true] {
            for colorspace in [
                ColorSpace::RGB,
                ColorSpace::RGBA,
                ColorSpace::BGR,
                ColorSpace::BGRA
            ] {
                let options = DecoderOptions::default()
                    .set_use_unsafe(use_simd)
                    .jpeg_set_out_colorspace(colorspace);
                let expected = packed(rgb, colorspace);
                for dnl in [false, true] {
                    let data = if dnl { with_dnl(jpeg, height) } else { jpeg.to_vec() };
                    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
                    assert_eq!(
                        decoder.decode().unwrap(),
                        expected,
                        "{width}x{height}, {colorspace:?}, SIMD={use_simd}, DNL={dnl}"
                    );
                    assert_eq!(decoder.dimensions(), Some((width, height)));

                    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options);
                    let mut output = vec![0xa5; expected.len() + 11];
                    let result = decoder.decode_into(&mut output[..expected.len()]);
                    if dnl {
                        // DNL caller storage must cover the configured maximum height.
                        assert!(
                            matches!(result, Err(DecodeErrors::TooSmallOutput(_, got)) if got == expected.len())
                        );
                        assert!(output.iter().all(|&byte| byte == 0xa5));
                    } else {
                        result.unwrap();
                        assert_eq!(&output[..expected.len()], expected);
                    }
                    assert_eq!(&output[expected.len()..], &[0xa5; 11]);
                }
                for batch in [1, 2, height] {
                    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(jpeg), options);
                    let mut sink = decoder.scanline_output();
                    let row_bytes = width * colorspace.num_components();
                    let stride = row_bytes + 7;
                    let mut output = vec![0xa5; stride * batch];
                    let mut actual = Vec::new();
                    let mut count = 0;
                    while count < height {
                        let available = batch.min(height - count);
                        let status = sink
                            .read_scanlines(&mut output[..stride * available], stride)
                            .unwrap();
                        let ScanlineReadStatus::RowsProcessed { rows } = status else {
                            panic!("unexpected scanline status: {status:?}");
                        };
                        assert!(rows > 0 && rows <= available);
                        for row in output[..rows * stride].chunks_exact(stride) {
                            actual.extend_from_slice(&row[..row_bytes]);
                            assert_eq!(&row[row_bytes..], &[0xa5; 7]);
                        }
                        count += rows;
                    }
                    assert_eq!(count, height);
                    assert_eq!(sink.finish().unwrap(), ScanlineStatus::Complete);
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
