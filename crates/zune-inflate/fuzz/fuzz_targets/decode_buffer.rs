#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here
    let mut decoder = zune_inflate::DeflateDecoder::new(data);
    let _ = decoder.decode_zlib();
});
