/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use crate::{hash, sample_path, TestEntry};

pub fn jpeg_path() -> PathBuf {
    sample_path().join("test-images/jpeg")
}

#[test]
#[allow(clippy::uninlined_format_args)]
fn test_jpeg() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/jpeg.json");

    let json_file = read(file).unwrap();

    let paths: Vec<TestEntry> = serde_json::from_slice(&json_file).unwrap();

    let default_path = jpeg_path();
    let mut error = false;
    let mut files = Vec::new();
    for path in &paths {
        let file_name = default_path.join(&path.name);

        let expected_hash = path.hash;

        // load file
        let file_contents = read(&file_name).unwrap();

        let mut options = DecoderOptions::default();

        if let Some(color) = path.colorspace {
            options = options.jpeg_set_out_colorspace(color.to_colorspace());
        }

        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&file_contents), options);
        let pixels = decoder.decode().unwrap();

        let hash = hash(&pixels);

        if hash != expected_hash {
            error = true;
            files.push(path.to_owned());
            // report error
            let err = format!(
                "Hash mismatch for file {:?}\nExpected {} but found {}\nConfig:{:#?}",
                file_name, expected_hash, hash, path
            );
            eprintln!("{}\n", err)
        }
    }
    if error {
        panic!("Errors found during test decoding\n {:#?}", files);
    }
}
