/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::path::{Path, PathBuf};

use zune_bmp::BmpDecoder;
use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;

use crate::{hash, sample_path, TestEntry};

pub fn bmp_path() -> PathBuf {
    sample_path().join("test-images/bmp")
}

#[test]
#[allow(clippy::uninlined_format_args)]
fn test_bmp() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bmp.json");

    let json_file = read(file).unwrap();

    let paths: Vec<TestEntry> = serde_json::from_slice(&json_file).unwrap();

    let default_path = bmp_path();
    let mut error = false;
    let mut files = Vec::new();
    for path in &paths {
        let file_name = default_path.join(&path.name);

        let expected_hash = path.hash;

        // load file
        let file_contents = ZCursor::new(read(&file_name).unwrap());

        let options = DecoderOptions::default();

        let mut decoder = BmpDecoder::new_with_options(file_contents, options);
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
            eprintln!("{}\n", err);
        }
    }
    if error {
        panic!("Errors found during test decoding\n {:#?}", files);
    }
}
