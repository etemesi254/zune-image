use std::fs;
use zune_core::bytestream::ZCursor;
use zune_core::options::{DecoderOptions, JpegScale};
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

    let scale = JpegScale::Eighth;
    let options = DecoderOptions::default().jpeg_set_scale(scale);
    let mut scaled_decoder = JpegDecoder::new_with_options(ZCursor::new(&bytes[..]), options);
    let scaled = scaled_decoder.decode().unwrap();
    let scaled_info = scaled_decoder.info().unwrap();
    let scaled_width = 8192_usize.div_ceil(scale.denominator());
    let scaled_height = 7524_usize.div_ceil(scale.denominator());

    assert_eq!(scaled_info.height, 7524, "DNL image: height must be set before scaling");
    assert_eq!(scaled.len(), scaled_width * scaled_height * 3);

    let full_stride = 8192 * 3;
    let scaled_stride = scaled_width * 3;
    for y in 0..scaled_height {
        let src_y = (y * scale.denominator()).min(7524 - 1);
        for x in 0..scaled_width {
            let src_x = (x * scale.denominator()).min(8192 - 1);
            let src = src_y * full_stride + src_x * 3;
            let dst = y * scaled_stride + x * 3;
            assert_eq!(&scaled[dst..dst + 3], &pixels[src..src + 3]);
        }
    }
}
