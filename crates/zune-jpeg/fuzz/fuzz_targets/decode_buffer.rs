#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use zune_jpeg::zune_core::bytestream::ZCursor;
    let data = ZCursor::new(data);
    let mut decoder = zune_jpeg::JpegDecoder::new(data);
    let _ = decoder.decode();
});
