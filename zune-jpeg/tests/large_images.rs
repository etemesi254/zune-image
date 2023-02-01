#![allow(clippy::field_reassign_with_default)]

use xxhash_rust::xxh3::xxh3_128;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

/// Decodes a large image
#[test]
fn large_no_sampling_factors_rgb()
{
    //
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/large_no_samp_7680_4320.jpg";
    let data = std::fs::read(path).unwrap();
    let pixels = JpegDecoder::new(&data).decode().unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 72227651583886420023815834092904494086;

    assert_eq!(hash, EXPECTED);
}

/// Decodes a large image
#[test]
fn large_vertical_sampling_factors_rgb()
{
    //
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/large_vertical_samp_7680_4320.jpg";
    let data = std::fs::read(path).unwrap();
    let pixels = JpegDecoder::new(&data).decode().unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 89624073440418279124423614502641624055;

    assert_eq!(hash, EXPECTED);
}
#[test]
fn large_no_sampling_factors_grayscale()
{
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/large_no_samp_7680_4320.jpg";

    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);

    let pixels = JpegDecoder::new_with_options(options, data)
        .decode()
        .unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 250332075509747503823959729866758307162;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn large_no_sampling_factors_ycbcr()
{
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/large_no_samp_7680_4320.jpg";

    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::YCbCr);
    let pixels = JpegDecoder::new_with_options(options, data)
        .decode()
        .unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 93348528356461250907756341851990389554;

    assert_eq!(hash, EXPECTED);
}

#[test]
fn large_horizontal_sampling_rgb()
{
    //
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/large_horiz_samp_7680_4320.jpg";
    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);

    let pixels = JpegDecoder::new_with_options(options, data)
        .decode()
        .unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 156938646890602880804753012205027071578;

    assert_eq!(hash, EXPECTED);
}
#[test]
fn large_horizontal_sampling_grayscale()
{
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/large_horiz_samp_7680_4320.jpg";
    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);

    let pixels = JpegDecoder::new_with_options(options, data)
        .decode()
        .unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 273130140778234680792798421363572621515;

    assert_eq!(hash, EXPECTED);
}
#[test]
fn large_horizontal_sampling_ycbcr()
{
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/large_horiz_samp_7680_4320.jpg";

    let data = &std::fs::read(path).unwrap();

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::YCbCr);

    let pixels = JpegDecoder::new_with_options(options, data)
        .decode()
        .unwrap();

    let hash = xxh3_128(&pixels);
    const EXPECTED: u128 = 192607723684787980167689860133863748259;

    assert_eq!(hash, EXPECTED);
}
