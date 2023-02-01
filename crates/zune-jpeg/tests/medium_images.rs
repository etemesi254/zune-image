#![allow(clippy::field_reassign_with_default)]

use xxhash_rust::xxh3::xxh3_128;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

/// Decodes a large image
#[test]
fn medium_no_sampling_factors_rgb()
{
    //
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/medium_no_samp_2500x1786.jpg";
    let data = std::fs::read(path).unwrap();

    let mut decoder = JpegDecoder::new(&data);

    let pixels = decoder.decode().expect("Test failed decoding");
    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 279295790094485170156316723198300362939;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn medium_no_sampling_factors_grayscale()
{
    //
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/medium_no_samp_2500x1786.jpg";
    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);

    let mut decoder = JpegDecoder::new_with_options(options, data);
    // Grayscale

    let pixels = decoder.decode().expect("Test failed decoding");

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 291874786663895286460461230469345392126;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn medium_horizontal_sampling_rgb()
{
    //
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/medium_horiz_samp_2500x1786.jpg";
    let data = &std::fs::read(path).unwrap();

    let mut decoder = JpegDecoder::new(data);

    let pixels = decoder.decode().expect("Test failed decoding");

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 125683957700914688041687332115454166076;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn medium_horizontal_sampling_grayscale()
{
    // Grayscale
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/medium_horiz_samp_2500x1786.jpg";
    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut decoder = JpegDecoder::new_with_options(options, data);

    let pixels = decoder.decode().expect("Test failed decoding");

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 291874786663895286460461230469345392126;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn medium_horizontal_sampling_cymk()
{
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/medium_horiz_samp_2500x1786.jpg";
    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::YCbCr);

    let mut decoder = JpegDecoder::new_with_options(options, data);
    // cymk

    let pixels = decoder.decode().expect("Test failed decoding");

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 289066310719258065604782525085636628338;

    assert_eq!(hash, EXPECTED);
}
