#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use zune_core::bytestream::ZCursor;
    let opts = zune_core::options::DecoderOptions::new_fast();

    let data = ZCursor::new(data);
    let mut decoder = zune_png::PngDecoder::new_with_options(data, opts);
    let _ = decoder.decode();
});
