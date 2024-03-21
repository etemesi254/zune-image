#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here

    use zune_psd::zune_core::bytestream::ZCursor;
    let data = ZCursor::new(data);

    let mut decoder = zune_psd::PSDDecoder::new(data);
    let _ = decoder.decode();
});
