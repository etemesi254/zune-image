#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut decoder = zune_png::PngDecoder::new(data);
    decoder.confirm_checksums(false);
    let _ = decoder.decode();
});
