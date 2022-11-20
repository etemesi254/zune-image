use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use zune_jpeg::JpegDecoder;
use zune_tests::{create_tests_from_dirs, write_to_file, TestTrait};

struct MyType;

impl TestTrait for MyType
{
    fn decode(&mut self, compressed_bytes: &[u8]) -> Vec<u8>
    {
        let mut decoder = JpegDecoder::new(compressed_bytes);

        decoder.decode_buffer().unwrap()
    }
}

fn filter(path: &PathBuf) -> bool
{
    let mut file = File::open(path).unwrap();
    let mut magic_bytes = [0; 2];
    file.read_exact(&mut magic_bytes).unwrap();

    magic_bytes == (0xffd8_u16).to_be_bytes()
}

#[test]
fn add_tests()
{
    let paths = env!("CARGO_MANIFEST_DIR").to_string();
    let extra = "/tests/inputs/";
    let mut t = MyType {};

    let tests = create_tests_from_dirs(paths.clone(), extra.to_string(), filter, &mut t);

    write_to_file(paths + "/tests/files.toml", &tests);

    dbg!(tests);
}
