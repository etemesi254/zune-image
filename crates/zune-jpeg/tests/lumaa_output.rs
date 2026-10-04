//! Asking for an output colorspace the decoder does not convert to returns an error, for
//! vertically subsampled images too (they used to panic carrying over chroma rows that were
//! never decoded).

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

#[test]
fn unsupported_lumaa_output_is_an_error_for_vertically_subsampled_images() {
    // 4:2:0 (2x2 luma sampling, chroma 1x1)
    let data = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::LumaA);
    let result = std::panic::catch_unwind(|| {
        JpegDecoder::new_with_options(ZCursor::new(data.as_slice()), options).decode()
    });
    assert!(result.is_ok(), "decoding to LumaA panicked");
    assert!(result.unwrap().is_err(), "LumaA output is not implemented");
}
