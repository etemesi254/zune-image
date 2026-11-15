use zune_core::bit_depth::ByteEndian;
use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_core::result::DecodingResult;
use zune_png::PngDecoder;

/// 16 bit samples as stored in the file (big endian)
fn decode_ref(data: &[u8]) -> Vec<u8> {
    let decoder = png::Decoder::new(ZCursor::new(data));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut buf).unwrap();
    buf
}

fn swap_pairs(data: &[u8]) -> Vec<u8> {
    data.chunks_exact(2).flat_map(|x| [x[1], x[0]]).collect()
}

fn check_byte_endian(name: &str) {
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/png_suite/" + name;
    let data = std::fs::read(path).unwrap();
    let big_endian = decode_ref(&data);

    for (endian, expected) in [
        (ByteEndian::BE, big_endian.clone()),
        (ByteEndian::LE, swap_pairs(&big_endian))
    ] {
        let options = DecoderOptions::default().set_byte_endian(endian);

        let raw = PngDecoder::new_with_options(ZCursor::new(&data), options)
            .decode_raw()
            .unwrap();
        assert!(raw == expected, "{name}: decode_raw with {endian:?}");

        let mut decoder = PngDecoder::new_with_options(ZCursor::new(&data), options);
        decoder.decode_headers().unwrap();
        let mut into = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.decode_into(&mut into).unwrap();
        assert!(into == expected, "{name}: decode_into with {endian:?}");

        // decode() returns native endian u16 samples whatever the configured byte order
        let decoded = PngDecoder::new_with_options(ZCursor::new(&data), options)
            .decode()
            .unwrap();
        let DecodingResult::U16(samples) = decoded else {
            panic!("{name}: expected 16 bit samples")
        };
        let native: Vec<u16> = big_endian
            .chunks_exact(2)
            .map(|x| u16::from_be_bytes([x[0], x[1]]))
            .collect();
        assert!(samples == native, "{name}: decode with {endian:?}");
    }
}

#[test]
fn sixteen_bit_samples_follow_byte_endian() {
    check_byte_endian("basn6a16.png");
}

#[test]
fn sixteen_bit_interlaced_samples_follow_byte_endian() {
    check_byte_endian("basi2c16.png");
}
