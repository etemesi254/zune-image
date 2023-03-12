use std::ffi::OsStr;
use std::fs::read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use zune_core::options::DecoderOptions;
use zune_inflate::DeflateDecoder;
use zune_psd::PSDDecoder;

use crate::{hash, sample_path, TestEntry};

pub fn inflate_path() -> PathBuf
{
    sample_path().join("test-images/inflate")
}

#[test]
#[allow(clippy::uninlined_format_args)]
fn test_inflate()
{
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/inflate.json");

    let json_file = read(file).unwrap();

    let paths: Vec<TestEntry> = serde_json::from_slice(&json_file).unwrap();

    let default_path = inflate_path();
    let mut error = false;
    let mut files = Vec::new();

    for path in &paths
    {
        let file_name = default_path.join(&path.name);

        let expected_hash = path.hash;

        // load file
        let file_contents = read(&file_name).unwrap();

        let mut decoder = DeflateDecoder::new(&file_contents);

        let pixels = if file_name.extension() == Some(OsStr::from_bytes(b"zlib"))
        {
            decoder.decode_zlib().unwrap()
        }
        else if file_name.extension() == Some(OsStr::from_bytes(b"gz"))
        {
            decoder.decode_gzip().unwrap()
        }
        else
        {
            todo!("Format {:?}", file_name.extension());
        };

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
