use std::fs::read;
use std::path::{Path, PathBuf};

use zune_core::options::DecoderOptions;
use zune_png::PngDecoder;

use crate::{hash, sample_path, TestEntry};

pub fn png_path() -> PathBuf
{
    sample_path().join("test-images/png")
}

#[test]
#[allow(clippy::uninlined_format_args)]
fn test_png()
{
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/png.json");

    let json_file = read(file).unwrap();

    let paths: Vec<TestEntry> = serde_json::from_slice(&json_file).unwrap();

    let default_path = png_path();
    let mut error = false;
    let mut files = Vec::new();
    for path in &paths
    {
        let file_name = default_path.join(&path.name);

        let expected_hash = path.hash;

        // load file
        let file_contents = read(&file_name).unwrap();

        let options = DecoderOptions::default();

        let mut decoder = PngDecoder::new_with_options(&file_contents, options);
        let pixels = decoder.decode_raw().unwrap();

        let hash = hash(&pixels);

        if hash != expected_hash
        {
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
    if error
    {
        panic!("Errors found during test decoding\n {:#?}", files);
    }
}
