#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut decoder = zune_jpeg::JpegDecoder::new(data);
    let _ = decoder.decode();
});
