use std::io::Cursor;

use zune_core::bytestream::ZCursor;
use zune_jpeg::errors::DecodeErrors;
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
