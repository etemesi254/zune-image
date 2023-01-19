#![allow(clippy::field_reassign_with_default)]

use xxhash_rust::xxh3::xxh3_128;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

#[test]
fn huffman_third_index()
{
    //
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/huffman_third_index.jpg";
    let data = &std::fs::read(path).unwrap();
    let mut options = DecoderOptions::default();
    options.out_colorspace = ColorSpace::Luma;
    let mut decoder = JpegDecoder::new_with_options(options, data);
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");
    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 133175253843308546331686378179836169848;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn single_qt()
{
    // This image has a single quantization header
    // with multiple QT tables defined.
    // Allows us to ensure that the multi-table QT handling logic works
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/single_qt.jpeg";
    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default();
    let mut decoder = JpegDecoder::new_with_options(options, data);
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");
    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 39914510576233829517731889017478170875;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn google_pixel()
{
    //
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/google_pixel.jpg";

    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default();
    let mut decoder = JpegDecoder::new_with_options(options, data);
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");
    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 297723773723904045145011723267592936554;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn google_pixel_progressive()
{
    //
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/google_pixel_progressive.jpg";

    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default();
    let mut decoder = JpegDecoder::new_with_options(options, data);
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");
    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 119912241572774598124330387855642941476;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn test_four_components()
{
    //
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/four_components.jpg";

    let data = &std::fs::read(path).unwrap();

    let mut decoder = JpegDecoder::new(data);
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 33350711354489254164962650813791563794;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn test_large_component_number()
{
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/huge_sof_number.jpg";
    let data = &std::fs::read(path).unwrap();

    let mut decoder = JpegDecoder::new(data);
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 46029048520010428617927201003495890872;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn test_basic()
{
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/test-images/weird_components.jpg";

    let data = &std::fs::read(path).unwrap();

    let pixels = JpegDecoder::new(data).decode().unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 56242265237748686029496710371134854998;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn test_four_components_progressive_jpeg()
{
    let path = env!("CARGO_MANIFEST_DIR").to_string()
        + "/test-images/Kiara_limited_progressive_four_components.jpg";
    let data = &std::fs::read(path).unwrap();

    let pixels = JpegDecoder::new(data).decode().unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 327899417203221693667684823644765405315;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn test_fill_bytes_before_marker()
{
    let path = env!("CARGO_MANIFEST_DIR").to_string()
        + "/test-images/rebuilt_relax_fill_bytes_before_marker.jpg";
    let data = &std::fs::read(path).unwrap();

    let pixels = JpegDecoder::new(data).decode().unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 293398073876105658422289501725766251280;

    assert_eq!(hash, EXPECTED);
}
