/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::error::Error;
use std::io::{self, Write};

use zune_core::bytestream::ZCursor;
use zune_jpeg::{JpegDecoder, JpegScale};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 3 {
        return Err("usage: scaled_decode image denominator".into());
    }
    let scale = match arguments[2].as_str() {
        "2" => JpegScale::Half,
        "4" => JpegScale::Quarter,
        "8" => JpegScale::Eighth,
        _ => return Err("unsupported denominator".into())
    };
    let data = std::fs::read(&arguments[1])?;
    let mut decoder = JpegDecoder::new(ZCursor::new(&data)).into_scaled(scale)?;
    let mut output = vec![0; decoder.output_buffer_size().ok_or("output size overflow")?];
    decoder.decode_into(&mut output)?;
    eprintln!("decoded {:?}", decoder.dimensions());
    io::stdout().write_all(&output)?;
    Ok(())
}
