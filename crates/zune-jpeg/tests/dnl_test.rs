use std::fs;
use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

#[test]
fn test_dnl_decoding() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/images/dnl_image.jpg");
    let bytes = fs::read(path).unwrap();
    let mut decoder = JpegDecoder::new(ZCursor::new(&bytes[..]));
    
    decoder.decode_headers().unwrap();
    let info = decoder.info().unwrap();
    assert_eq!(info.width, 8192);
    assert_eq!(info.height, 7524);
    
    let pixels = decoder.decode().unwrap();
    assert_eq!(pixels.len(), 8192 * 7524 * 3);
}
