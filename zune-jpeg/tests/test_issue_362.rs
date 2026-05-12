// Test for issue #362: range start index out of range in idct_int_1x1
// https://github.com/etemesi254/zune-image/issues/362

use zune_jpeg::JpegDecoder;

#[test]
fn test_issue_362_idct_bounds() {
    // Fuzz-generated JPEG that triggers panic in idct/scalar.rs:36
    let data = include_bytes!("../fuzz_test_362.jpg");

    let mut decoder = JpegDecoder::new(data);

    // Should return an error instead of panicking
    let result = decoder.decode();

    // We expect this to either succeed or return a proper error,
    // not panic with "range start index out of range"
    match result {
        Ok(_) => {
            // If it decodes successfully, that's fine
        }
        Err(_) => {
            // If it returns an error, that's also acceptable
            // The key is that it should NOT panic
        }
    }
}
