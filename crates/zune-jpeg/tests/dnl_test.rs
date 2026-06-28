use std::fs;
use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

#[test]
fn test_dnl_decoding() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/images/dnl_image.jpg");
    let bytes = fs::read(path).unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(&bytes[..]));

    // After decode_headers(), height is 0 for DNL images because the actual
    // number of lines is not known until the entropy data has been decoded.
    // Width is always available from the SOF header.
    decoder.decode_headers().unwrap();
    let info = decoder.info().unwrap();
    assert_eq!(info.width, 8192);
    assert_eq!(info.height, 0, "DNL image: height must be 0 after decode_headers (real height is in the DNL marker)");

    // decode() runs entropy decoding, intercepts the DNL marker, and returns
    // the correctly-sized pixel buffer.
    let pixels = decoder.decode().unwrap();
    let info_after = decoder.info().unwrap();
    assert_eq!(info_after.height, 7524, "DNL image: height must be set after decode()");
    assert_eq!(pixels.len(), 8192 * 7524 * 3);
}
