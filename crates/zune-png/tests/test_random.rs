use std::fs::read;
use std::path::Path;

fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8>
{
    read(path).unwrap()
}

fn decode_ref(data: &[u8]) -> Vec<u8>
{
    let decoder = png::Decoder::new(data);

    let mut reader = decoder.read_info().unwrap();

    // Allocate the output buffer.
    let mut buf = vec![0; reader.output_buffer_size()];
    // Read the next frame. An APNG might contain multiple frames.
    let _ = reader.next_frame(&mut buf).unwrap();

    buf
}

fn decode_zune(data: &[u8]) -> Vec<u8>
{
    zune_png::PngDecoder::new(data)
        .decode()
        .unwrap()
        .u8()
        .unwrap()
}

fn test_decoding<P: AsRef<Path>>(path: P)
{
    let contents = open_and_read(path);

    let zune_results = decode_zune(&contents);
    let ref_results = decode_ref(&contents);
    assert_eq!(&zune_results, &ref_results);
}

#[test]
fn test_trns_transparency()
{
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/random/xc.png";

    test_decoding(path);
}
