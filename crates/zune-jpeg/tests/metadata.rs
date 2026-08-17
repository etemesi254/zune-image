use std::io::Cursor;

use zune_core::bytestream::ZCursor;
use zune_jpeg::errors::{DecodeErrors, UnsupportedSchemes};
use zune_jpeg::JpegDecoder;

fn grayscale_fixture(sof_marker: u8, sample_precision: u8) -> Vec<u8> {
    let mut data =
        include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg").to_vec();
    let sof = data
        .windows(2)
        .position(|bytes| bytes == [0xFF, 0xC2])
        .expect("fixture must contain SOF2");

    data[sof + 1] = sof_marker;
    data[sof + 4] = sample_precision;
    data[sof + 5..sof + 7].copy_from_slice(&32_u16.to_be_bytes());
    data[sof + 7..sof + 9].copy_from_slice(&32_u16.to_be_bytes());
    data[sof + 11] = 0x11;

    let sos = data[sof..]
        .windows(2)
        .position(|bytes| bytes == [0xFF, 0xDA])
        .map(|offset| sof + offset)
        .expect("fixture must contain SOS");
    let sos_length = usize::from(u16::from_be_bytes([data[sos + 2], data[sos + 3]]));
    if sof_marker == 0xC1 {
        let scan_parameters = sos + 2 + sos_length - 3;
        data[scan_parameters..scan_parameters + 3].copy_from_slice(&[0, 63, 0]);
    }
    data.truncate(sos + 2 + sos_length);
    data
}

fn assert_twelve_bit_size(sof_marker: u8) {
    let data = grayscale_fixture(sof_marker, 12);
    let mut decoder = JpegDecoder::new(ZCursor::new(&data));

    decoder
        .decode_headers()
        .expect("12-bit Huffman headers should parse");
    assert_eq!(decoder.dimensions(), Some((32, 32)));

    let mut output = vec![0; decoder.output_buffer_size().unwrap()];
    let error = decoder
        .decode_into(&mut output)
        .expect_err("12-bit pixel decoding must remain unsupported");
    assert!(error.to_string().contains("12-bit"));
    assert_eq!(decoder.dimensions(), Some((32, 32)));
}

fn lossless_header(precision: u8, sampling: u8, quantization_table: u8) -> Vec<u8> {
    vec![
        0xFF, 0xD8, // SOI
        0xFF, 0xC4, 0x00, 0x14, // DHT, one DC symbol
        0x00, // DC table 0
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // code counts 1-8
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // code counts 9-16
        0x10, // lossless difference category 16
        0xFF, 0xC3, 0x00, 0x0B, precision, 0x00, 0x20, 0x00, 0x20, 0x01,
        0x01, sampling, quantization_table, // component
        0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x01, 0x00, 0x00, // SOS
        0xFF, 0xD9 // EOI
    ]
}

fn lossless_arithmetic_header(component_count: u8, interleaved: bool) -> Vec<u8> {
    let mut data = vec![0xFF, 0xD8, 0xFF, 0xCB]; // SOI, SOF11
    let sof_length = 8 + 3 * u16::from(component_count);
    data.extend_from_slice(&sof_length.to_be_bytes());
    data.extend_from_slice(&[8, 0, 32, 0, 32, component_count]);
    for component in 1..=component_count {
        data.extend_from_slice(&[component, 0x11, 0]);
    }
    data.extend_from_slice(&[0xFF, 0xCC, 0, 4, 0, 0x10]); // DAC, DC table 0

    let scans: Vec<Vec<u8>> = if interleaved {
        vec![(1..=component_count).collect()]
    } else {
        (1..=component_count).map(|component| vec![component]).collect()
    };
    for scan in scans {
        data.extend_from_slice(&[0xFF, 0xDA]);
        let sos_length = 6 + 2 * u16::try_from(scan.len()).unwrap();
        data.extend_from_slice(&sos_length.to_be_bytes());
        data.push(u8::try_from(scan.len()).unwrap());
        for component in scan {
            data.extend_from_slice(&[component, 0]);
        }
        data.extend_from_slice(&[1, 0, 0]);
    }
    data.extend_from_slice(&[0xFF, 0xD9]);
    data
}

#[test]
fn lossless_huffman_headers_expose_dimensions() {
    for precision in [2, 8, 12, 16] {
        let data = lossless_header(precision, 0x21, 1);
        let mut decoder = JpegDecoder::new(ZCursor::new(&data));

        decoder.decode_headers().unwrap();

        let info = decoder.info().unwrap();
        assert_eq!(decoder.dimensions(), Some((32, 32)));
        assert_eq!(info.pixel_density, precision);
        assert!(info.sof.is_lossless());
    }
}

#[test]
fn lossless_huffman_pixel_decode_remains_unsupported() {
    let data = lossless_header(8, 0x11, 0);
    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    decoder.decode_headers().unwrap();
    let mut output = vec![0xA5; decoder.output_buffer_size().unwrap()];

    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(
        error,
        DecodeErrors::Unsupported(UnsupportedSchemes::LosslessHuffman)
    ));
    assert!(output.iter().all(|&byte| byte == 0xA5));

    let error = JpegDecoder::new(ZCursor::new(&data)).decode().unwrap_err();
    assert!(matches!(
        error,
        DecodeErrors::Unsupported(UnsupportedSchemes::LosslessHuffman)
    ));
}

#[test]
fn lossless_arithmetic_headers_expose_dimensions() {
    for (component_count, interleaved) in [(1, true), (3, false), (3, true)] {
        let data = lossless_arithmetic_header(component_count, interleaved);
        let mut decoder = JpegDecoder::new(ZCursor::new(&data));

        decoder.decode_headers().unwrap();

        let info = decoder.info().unwrap();
        assert_eq!(decoder.dimensions(), Some((32, 32)));
        assert_eq!(info.components, component_count);
        assert!(info.sof.is_lossless());

        let error = decoder.decode().unwrap_err();
        assert!(matches!(
            error,
            DecodeErrors::Unsupported(UnsupportedSchemes::LosslessArithmetic)
        ));
    }
}

#[test]
fn lossless_only_huffman_categories_stay_rejected_for_lossy_frames() {
    let mut data = lossless_header(8, 0x11, 0);
    let sof = data.windows(2).position(|bytes| bytes == [0xFF, 0xC3]).unwrap();
    data[sof + 1] = 0xC0;

    let error = JpegDecoder::new(ZCursor::new(&data))
        .decode_headers()
        .unwrap_err();
    assert!(matches!(error, DecodeErrors::HuffmanDecode(_)));
}

#[test]
fn iptc_metadata() {
    const EXPECTED_DATA: &[u8] = &[
        56, 66, 73, 77, 4, 4, 0, 0, 0, 0, 0, 99, 28, 2, 90, 0, 8, 66, 117, 100, 97, 112, 101,
        115, 116, 28, 2, 101, 0, 7, 72, 117, 110, 103, 97, 114, 121, 28, 2, 25, 0, 3, 72, 118, 75,
        28, 2, 25, 0, 4, 50, 48, 48, 54, 28, 2, 25, 0, 6, 115, 117, 109, 109, 101, 114, 28, 2, 25,
        0, 4, 74, 117, 108, 121, 28, 2, 25, 0, 7, 104, 111, 108, 105, 100, 97, 121, 28, 2, 25, 0,
        7, 72, 117, 110, 103, 97, 114, 121, 28, 2, 25, 0, 8, 66, 117, 100, 97, 112, 101, 115, 116,
        0,
    ];
    let image: &[u8] = include_bytes!("images/iptc.jpeg");

    let mut decoder = JpegDecoder::new(Cursor::new(image));
    decoder.decode_headers().unwrap();
    assert_eq!(decoder.iptc(), Some(&EXPECTED_DATA.to_vec()))
}

#[test]
fn extended_xmp() {
    const EXPECTED_GUID: &[u8] = &[
        50, 67, 69, 57, 54, 51, 49, 68, 57, 48, 69, 52, 57, 67, 52, 50, 67, 70, 48, 54, 49, 52, 52,
        51, 48, 49, 68, 52, 53, 56, 48, 57
    ];

    let image: &[u8] = include_bytes!("../../../test-images/jpeg/2029_extended_xmp.jpg").as_slice();
    let mut decoder = JpegDecoder::new(Cursor::new(image));
    decoder.decode_headers().unwrap();

    assert_eq!(
        decoder.info().unwrap().extended_xmp_guid,
        Some(EXPECTED_GUID.to_vec())
    );
    assert_eq!(decoder.info().unwrap().extended_xmp.unwrap().len(), 75718);
}

#[test]
fn test_sample_ratios() {
    use zune_jpeg::SampleRatios;

    let images = [
        (
            include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg").as_slice(),
            SampleRatios::None,
        ),
        (
            include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg").as_slice(),
            SampleRatios::HV,
        ),
        (
            include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg").as_slice(),
            SampleRatios::H,
        ),
        (
            include_bytes!("../../../test-images/jpeg/non_interleaved_440_64x64.jpg").as_slice(),
            SampleRatios::V,
        ),
    ];

    for (data, expected) in images {
        let mut decoder = JpegDecoder::new(Cursor::new(data));
        decoder.decode_headers().unwrap();

        let info = decoder.info().unwrap();
        assert_eq!(
            info.sample_ratio, expected,
            "Expected sample ratio {expected:?} for image"
        );
    }
}

#[test]
fn extended_huffman_12_bit_headers() {
    assert_twelve_bit_size(0xC1);
}

#[test]
fn progressive_huffman_12_bit_headers() {
    assert_twelve_bit_size(0xC2);
}

#[test]
fn baseline_12_bit_headers_remain_rejected() {
    let data = grayscale_fixture(0xC0, 12);
    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    let error = decoder
        .decode_headers()
        .expect_err("12-bit baseline SOF0 is not a supported header format");
    assert!(matches!(error, DecodeErrors::SofError(_)));
}

#[test]
fn extended_huffman_8_bit_decode_remains_supported() {
    let baseline = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let mut extended = baseline.to_vec();
    let sof = extended
        .windows(2)
        .position(|bytes| bytes == [0xFF, 0xC0])
        .expect("fixture must contain SOF0");
    extended[sof + 1] = 0xC1;

    let mut baseline_decoder = JpegDecoder::new(ZCursor::new(baseline));
    let expected = baseline_decoder
        .decode()
        .expect("SOF0 fixture should decode");

    let mut decoder = JpegDecoder::new(ZCursor::new(&extended));
        decoder
            .decode_headers()
            .expect("8-bit SOF1 headers should parse");

    let actual = decoder.decode().expect("8-bit SOF1 pixels should decode");
    assert_eq!(actual, expected);
}
