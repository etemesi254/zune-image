use zune_ppm::zune_core::bytestream::ZCursor;
use zune_ppm::zune_core::result::DecodingResult;
use zune_ppm::PPMDecoder;

fn decode(data: &[u8]) -> DecodingResult {
    PPMDecoder::new(ZCursor::new(data)).decode().unwrap()
}

#[test]
fn test_raster_starting_with_whitespace() {
    // raster bytes that look like whitespace or a comment are pixel values
    for &first in b"\t\n\r #" {
        let raster = [first, b'\n', b'#', 7, b' ', 0];

        for header in [
            &b"P5\n3 2\n255\n"[..],
            b"P5\n3 2\n255#comment\n",
            b"P6\n1 2\n255\n",
            b"P7\nWIDTH 3\nHEIGHT 2\nDEPTH 1\nMAXVAL 255\nTUPLTYPE GRAYSCALE\nENDHDR\n"
        ] {
            let pixels = decode(&[header, &raster].concat()).u8().unwrap();
            assert_eq!(pixels, raster, "{:?}", String::from_utf8_lossy(header));
        }
        let pixels = decode(&[&b"P5\n3 1\n65535\n"[..], &raster].concat());
        let expected: Vec<u16> = raster
            .chunks_exact(2)
            .map(|x| u16::from_be_bytes([x[0], x[1]]))
            .collect();
        assert_eq!(pixels.u16().unwrap(), expected);

        let value = f32::from_le_bytes([first, 0, 0, 0x3f]);
        match decode(&[&b"Pf\n1 1\n-1.0\n"[..], &value.to_le_bytes()].concat()) {
            DecodingResult::F32(pixels) => assert_eq!(pixels, [value]),
            _ => panic!("expected f32 pixels")
        }
    }
}
