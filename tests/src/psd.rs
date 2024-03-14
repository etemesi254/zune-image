/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::path::{Path, PathBuf};

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_psd::PSDDecoder;

use crate::{hash, sample_path, TestEntry};

pub fn psd_path() -> PathBuf {
    sample_path().join("test-images/psd")
}

#[test]
#[allow(clippy::uninlined_format_args)]
fn test_psd() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/psd.json");

    let json_file = read(file).unwrap();

    let paths: Vec<TestEntry> = serde_json::from_slice(&json_file).unwrap();

    let default_path = psd_path();
    let mut error = false;
    let mut files = Vec::new();
    for path in &paths {
        let file_name = default_path.join(&path.name);

        let expected_hash = path.hash;

        // load file
        let file_contents = read(&file_name).unwrap();

        let options = DecoderOptions::default();

        let mut decoder = PSDDecoder::new_with_options(ZCursor::new(&file_contents), options);
        let pixels = decoder.decode_raw().unwrap();

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
