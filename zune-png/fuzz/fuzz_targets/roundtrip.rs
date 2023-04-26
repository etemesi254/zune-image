/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![no_main]

use libfuzzer_sys::fuzz_target;
use png::{BitDepth, ColorType, FilterType};

fuzz_target!(|data: (u8, u8, u8, u8, u8, Vec<u8>)| {
    if let Some((raw, encoded)) = encode_png(data.0, data.1, data.2, data.3, data.4, &data.5)
    {
        let raw_decoded = decode_png(&encoded);
        assert_eq!(raw, raw_decoded);
    }
});

fn encode_png(
    width: u8, filter: u8, compression: u8, color_type: u8, bit_depth: u8, data: &[u8]
) -> Option<(&[u8], Vec<u8>)>
{
    // Convert untyped bytes to the correct types and validate them:
    let width = width as u32;
    if width == 0
    {
        return None;
    };
    let filter = FilterType::from_u8(filter)?;
    if bit_depth < 8
    {
        return None;
    } // TODO: remove this hack
    let bit_depth = BitDepth::from_u8(bit_depth)?;
    let color_type = ColorType::from_u8(color_type)?;
    if let ColorType::Indexed = color_type
    {
        return None; // TODO: palette needs more data, not supported yet
    }
    // compression
    let compression = match compression
    {
        0 => png::Compression::Default,
        1 => png::Compression::Fast,
        2 => png::Compression::Best,
        3 => png::Compression::Huffman,
        4 => png::Compression::Rle,
        _ => return None
    };

    // infer the rest of the parameters
    let bytes_per_row = raw_row_length_from_width(bit_depth, color_type, width);
    // not the faintest clue why this -1 is needed but it is needed for bit depths other than 8
    // because otherwise the encoder will reject the input:
    // https://github.com/image-rs/image-png/blob/28035fd57312c29b38db5988fe84135de2d50e5d/src/encoder.rs#L657
    let bytes_per_row = bytes_per_row - 1;
    let height = data.len() / bytes_per_row;
    let total_bytes = bytes_per_row * height;
    let data_to_encode = &data[..total_bytes];

    // perform the PNG encoding
    let mut output: Vec<u8> = Vec::new();
    {
        // scoped so that we could return the Vec
        let mut encoder = png::Encoder::new(&mut output, width, height as u32);
        encoder.set_depth(bit_depth);
        encoder.set_color(color_type);
        encoder.set_filter(filter);
        encoder.set_compression(compression);
        // write_header_fn will return an error given invalid parameters,
        // such as height 0, or invalid color mode and bit depth combination
        let mut writer = encoder.write_header().ok()?;
        writer
            .write_image_data(data_to_encode)
            .expect("Encoding failed");
    }
    Some((data_to_encode, output))
}

fn decode_png(data: &[u8]) -> Vec<u8>
{
    zune_png::PngDecoder::new(data)
        .decode_raw()
        .expect("Failed to decode valid input data!")
}

// copied from the `png` codebase because it's pub(crate)
fn raw_row_length_from_width(depth: BitDepth, color: ColorType, width: u32) -> usize
{
    let samples = width as usize * color.samples();
    1 + match depth
    {
        BitDepth::Sixteen => samples * 2,
        BitDepth::Eight => samples,
        subbyte =>
        {
            let samples_per_byte = 8 / subbyte as usize;
            let whole = samples / samples_per_byte;
            let fract = usize::from(samples % samples_per_byte > 0);
            whole + fract
        }
    }
}
