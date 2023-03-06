#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() > 10
    {
        let compression_level = data[0];
        let data = &data[1..];
        let orig_len = data.len();
        let compressed = miniz_oxide::deflate::compress_to_vec(data, compression_level);
        let options = zune_inflate::DeflateOptions::default().set_limit(orig_len);
        let mut decoder = zune_inflate::DeflateDecoder::new_with_options(&compressed, options);
        let decoded = decoder
            .decode_deflate()
            .expect("Failed to decompress valid compressed data!");
        assert!(
            data == decoded,
            "The decompressed data doesn't match the original data!"
        );
    }
});
