use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

fn buffers_all_coefficients(jpeg: &[u8]) -> bool {
    let mut decoder = JpegDecoder::new(ZCursor::new(jpeg));
    decoder.decode_headers().expect("valid JPEG headers");
    let after_headers = decoder
        .buffers_all_coefficients()
        .expect("headers are decoded");
    decoder.decode().expect("valid JPEG");
    assert_eq!(decoder.buffers_all_coefficients(), Some(after_headers));
    after_headers
}

#[test]
fn buffers_all_coefficients_unavailable_before_headers() {
    let jpeg = include_bytes!("../../../test-images/jpeg/progressive_444_65x67.jpg");
    let decoder = JpegDecoder::new(ZCursor::new(&jpeg[..]));
    assert_eq!(decoder.buffers_all_coefficients(), None);
}

#[test]
fn progressive_image_buffers_all_coefficients() {
    let jpeg = include_bytes!("../../../test-images/jpeg/progressive_444_65x67.jpg");
    assert!(buffers_all_coefficients(jpeg));
}

#[test]
fn baseline_image_with_every_component_in_first_scan_does_not_buffer_all_coefficients() {
    let jpeg = include_bytes!("../../../test-images/jpeg/app14/baseline_rgb_interleaved.jpg");
    assert!(!buffers_all_coefficients(jpeg));
}

#[test]
fn baseline_image_with_one_component_in_first_scan_buffers_all_coefficients() {
    let jpeg = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    assert!(buffers_all_coefficients(jpeg));
}
