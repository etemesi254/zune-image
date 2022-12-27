#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut decoder = zune_inflate::DeflateDecoder::new(data);
    let _result = decoder.decode_zlib();
});
