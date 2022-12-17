use std::fs::read;
use std::io::prelude::*;
use std::io::Cursor;

fn decode_writer_flate(bytes: &[u8]) -> Vec<u8>
{
    let mut writer = Vec::new();

    let mut deflater = flate2::read::ZlibDecoder::new(Cursor::new(bytes));

    deflater.read_to_end(&mut writer).unwrap();

    writer
}

fn decode_writer_zune(bytes: &[u8]) -> Vec<u8>
{
    let mut deflater = zune_inflate::DeflateDecoder::new(bytes);

    deflater.decode_zlib().unwrap()
}

#[test]
fn test_similarity()
{
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/zlib";

    let dirs = std::fs::read_dir(path).unwrap();

    for file in dirs.flatten()
    {
        if file.path().is_file()
        {
            let data = read(&file.path()).unwrap();
            let zune_data = decode_writer_zune(&data);
            let flate_data = decode_writer_flate(&data);

            assert_eq!(zune_data.len(), flate_data.len());
            for ((pos, a), b) in zune_data.iter().enumerate().zip(flate_data.iter())
            {
                if a != b
                {
                    println!("FILE: {:?}", file.path());
                    panic!("[position: {pos}]: {a} {b} do not match");
                }
            }
        }
    }
}
