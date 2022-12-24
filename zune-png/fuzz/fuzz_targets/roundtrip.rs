#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some((raw, encoded)) = encode_png(data) {
        let raw_decoded = decode_png(&encoded);
        assert_eq!(raw, raw_decoded);
    }
});

const METADATA_BYTES: usize = 3;

fn encode_png(data: &[u8]) -> Option<(&[u8], Vec<u8>)> {
    // we use the first few bytes of the fuzzer input
    // to randomize the PNG encoding settings
    // so we need at least that much to be present
    if data.len() <= METADATA_BYTES {
        return None;
    }

    // randomize height
    let height = data[0] as u32; // max 255
    if height == 0 { return None }
    // randomize filter
    let filter = png::FilterType::from_u8(data[1])?;
    // randomize compression
    let compression = data[2];
    let compression = match compression {
        0 => png::Compression::Default,
        1 => png::Compression::Fast,
        2 => png::Compression::Best,
        3 => png::Compression::Huffman,
        4 => png::Compression::Rle,
        _ => return None,
    };

    // infer the rest of the parameters
    let bytes_per_pixel = 4;
    let data = &data[METADATA_BYTES..];
    let width = data.len() as u32 / height / bytes_per_pixel;
    if width == 0 { return None }
    let total_pixels = height.checked_mul(width)?;
    let total_bytes = total_pixels.checked_mul(bytes_per_pixel)?;

    // perform the PNG encoding
    let data_to_encode = &data[..total_bytes as usize];
    let mut output: Vec<u8> = Vec::new();
    { // scoped so that we could return the resulting Vec at the end
        let mut encoder = png::Encoder::new(&mut output, width, height);
        // TODO: randomize these settings
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_filter(filter);
        encoder.set_compression(compression);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data_to_encode).expect("Encoding failed");
    }

    Some((data_to_encode, output))
}

fn decode_png(data: &[u8]) -> Vec<u8> {
    let result = zune_png::PngDecoder::new(data).decode()
        .expect("Failed to decode valid input data!");
    // so far we only pass 8bpp RGBA values, so the result should always be u8
    result.u8().unwrap()
}