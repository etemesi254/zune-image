use std::io::Cursor;

use zune_jpeg::JpegDecoder;

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
