#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here

    use zune_hdr::zune_core::bytestream::ZCursor;
    let data = ZCursor::new(data);
    let mut decoder = zune_hdr::HdrDecoder::new(data);
    let _ = decoder.decode();
});
