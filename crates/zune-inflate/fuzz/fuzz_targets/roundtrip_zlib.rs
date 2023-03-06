#![no_main]

// This target uses zlib-ng to provide some variety in encoders.

// It is a good idea to run the `roundtrip` target first,
// which will use pure-Rust `miniz_oxide` which will
// be visible to the fuzzer, while zlib-ng is a black box
// because it is written in C.

// Copy the contents of `corpus/roundtrip` to `corpus/roundtrip_zlib`
// to kickstart the fuzzing with a decent corpus,
// which would be difficult to build with zlib-ng alone.

use std::io::Write;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() > 4
    {
        let compression_level = flate2::Compression::new((data[0] & 7).into());
        let data = &data[1..];
        let orig_len = data.len();
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), compression_level);
        e.write_all(data).unwrap();
        let compressed = e.finish().unwrap();
        let options = zune_inflate::DeflateOptions::default().set_limit(orig_len);
        let mut decoder = zune_inflate::DeflateDecoder::new_with_options(&compressed, options);
        let decoded = decoder
            .decode_zlib()
            .expect("Failed to decompress valid compressed data!");
        assert!(
            data == decoded,
            "The decompressed data doesn't match the original data!"
        );
    }
});
