use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_heic::{HeicErrors, HeifDecoder};

fn sample(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/fuzz-samples/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn decoder(data: &[u8]) -> HeifDecoder<ZCursor<&[u8]>> {
    // software decoding, so the result does not depend on the platform
    let options = DecoderOptions::default().hvec_set_use_videotoolbox(false);
    HeifDecoder::new_with_options(ZCursor::new(data), options)
}

#[test]
fn decode_into_matches_decode() {
    // a single image and a grid image
    for name in ["sample1.heif", "IMG_4862.HEIC"] {
        let data = sample(name);
        let expected = decoder(&data).decode().unwrap();

        let mut d = decoder(&data);
        d.decode_headers().unwrap();
        let size = d.output_buffer_size().unwrap();
        assert_eq!(size, expected.len(), "{name}");
        assert_eq!(size, d.width().unwrap() * d.height().unwrap() * d.colorspace().unwrap().num_components());

        // a larger buffer: only the first `size` bytes are written
        let mut out = vec![0xAB; size + 16];
        d.decode_into(&mut out).unwrap();
        assert!(out[..size] == expected[..], "{name}: pixels differ from decode()");
        assert!(out[size..].iter().all(|&b| b == 0xAB), "{name}: wrote past the image");
    }
}

#[test]
fn decode_into_rejects_small_buffer() {
    let data = sample("sample1.heif");
    let mut d = decoder(&data);
    d.decode_headers().unwrap();
    let size = d.output_buffer_size().unwrap();
    let mut out = vec![0; size - 1];
    match d.decode_into(&mut out) {
        Err(HeicErrors::TooSmallOutput(expected, found)) => {
            assert_eq!((expected, found), (size, size - 1));
        }
        other => panic!("expected TooSmallOutput, got {other:?}"),
    }
}

#[test]
fn output_buffer_size_needs_headers() {
    let data = sample("sample1.heif");
    assert_eq!(decoder(&data).output_buffer_size(), None);
}
