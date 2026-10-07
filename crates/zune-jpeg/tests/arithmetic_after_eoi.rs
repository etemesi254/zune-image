//! Bytes after the EOI marker must not change an arithmetic-coded image.
//!
//! When the entropy data of a scan stops short, the arithmetic decoder finishes the scan
//! reading zeros, as libjpeg-turbo does. Once it reaches EOI, nothing that follows (another
//! JPEG, as in Ultra HDR files, or any trailing bytes) belongs to the image.
#![cfg(feature = "arith")]

use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

fn decode(data: &[u8]) -> Vec<u8> {
    JpegDecoder::new(ZCursor::new(data)).decode().unwrap()
}

#[test]
fn bytes_after_eoi_do_not_change_a_short_arithmetic_scan() {
    let file = include_bytes!("../../../test-images/jpeg/arith/seq.jpg").as_slice();
    for cut in [600, 1200] {
        // the scan's data stops after `cut` bytes, followed by EOI
        let mut short = file[..cut].to_vec();
        short.extend_from_slice(&[0xff, 0xd9]);
        let expected = decode(&short);

        for (name, tail) in [
            ("0xAA bytes", vec![0xaa; 64]),
            ("a second JPEG", file.to_vec())
        ] {
            let mut with_tail = short.clone();
            with_tail.extend_from_slice(&tail);

            assert!(
                decode(&with_tail) == expected,
                "scan cut at {cut} bytes: {name} after EOI changed the image"
            );
        }
    }
}
