#![cfg(test)]

use nanorand::Rng;
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;

use crate::core_filters::colorspace::ColorspaceConv;
use crate::image::Image;
use crate::traits::OperationsTrait;

#[test]
fn test_cmyk_to_rgb() {
    let mut image = Image::fill(231_u8, ColorSpace::CMYK, 100, 100);
    // just confirm it works and hits the right path
    image.convert_color(ColorSpace::RGB).unwrap();
}

#[test]
fn test_rgb_to_cmyk() {
    let mut image = Image::fill(231_u8, ColorSpace::RGB, 100, 100);
    // just confirm it works and hits the right path
    image.convert_color(ColorSpace::CMYK).unwrap();
}

#[test]
fn test_real_time_rgb_to_cmyk() {
    use zune_core::options::DecoderOptions;
    use zune_jpeg::JpegDecoder;

    use crate::traits::DecoderTrait;

    // checks if conversion for cmyk to rgb holds for jpeg and this routine
    let mut file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // remove /zune-image
    file.pop();
    // remove /crates
    file.pop();
    let actual_file = file.join("test-images/jpeg/cymk.jpg");
    let data = std::fs::read(&actual_file).unwrap();
    // tell jpeg to output to cmyk
    let opts = DecoderOptions::new_fast().jpeg_set_out_colorspace(ColorSpace::CMYK);
    // set it up
    let decoder = JpegDecoder::new_with_options(ZCursor::new(&data), opts);
    let mut c: Box<dyn DecoderTrait> = Box::new(decoder);
    let mut im = c.decode().unwrap();
    // just confirm that this is good
    assert_eq!(im.colorspace(), ColorSpace::CMYK);
    // then convert it to rgb
    im.convert_color(ColorSpace::RGB).unwrap();
    // read the same image as rgb
    let new_img = Image::open(&actual_file).unwrap();

    assert!(new_img == im, "RGB to CYMK failed or diverged");
}

fn test_helper(im1: &Image, im2: &Image, im3: &Image, color: ColorSpace) {
    let filter = ColorspaceConv::new(color);
    filter
        .clone_and_execute(im1)
        .unwrap_or_else(|e| panic!("Could not convert to {:?} for im1, reason: {:?}", color, e));
    filter
        .clone_and_execute(im2)
        .unwrap_or_else(|e| panic!("Could not convert to {:?} for im2, reason: {:?}", color, e));
    filter
        .clone_and_execute(im3)
        .unwrap_or_else(|e| panic!("Could not convert to {:?} for im3, reason: {:?}", color, e));
}

fn create_image(color_space: ColorSpace) -> [Image; 3] {
    let mut rand = nanorand::WyRand::new();
    let f32_image = Image::fill(rand.generate::<f32>(), color_space, 100, 100);
    let u16_image = Image::fill(rand.generate::<u16>(), color_space, 100, 100);
    let u8_image = Image::fill(rand.generate::<u8>(), color_space, 100, 100);

    [u8_image, u16_image, f32_image]
}

fn single_tests(u8_im: &Image, u16_im: &Image, f32_im: &Image) {
    test_helper(u8_im, u16_im, f32_im, ColorSpace::RGB);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::RGBA);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::Luma);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::LumaA);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::CMYK);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::BGR);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::BGRA);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::ARGB);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::HSL);
    test_helper(u8_im, u16_im, f32_im, ColorSpace::HSV);
}
#[test]
fn test_rgb_to_other_colors() {
    let [u8_im, u16_im, f32_im] = create_image(ColorSpace::RGB);

    single_tests(&u8_im, &u16_im, &f32_im);
}

#[test]
fn test_rgba_to_other_colors() {
    let [u8_im, u16_im, f32_im] = create_image(ColorSpace::RGBA);

    single_tests(&u8_im, &u16_im, &f32_im);
}

#[test]
fn test_bgra_to_other_colors() {
    let [u8_im, u16_im, f32_im] = create_image(ColorSpace::BGRA);

    single_tests(&u8_im, &u16_im, &f32_im);
}

#[test]
fn test_bgr_to_other_colors() {
    let [u8_im, u16_im, f32_im] = create_image(ColorSpace::BGR);
    single_tests(&u8_im, &u16_im, &f32_im);
}

#[test]
fn test_hsl_to_other_colors() {
    let [u8_im, u16_im, f32_im] = create_image(ColorSpace::HSL);
    single_tests(&u8_im, &u16_im, &f32_im);
}
#[test]
fn test_luma_to_other_colors() {
    let [u8_im, u16_im, f32_im] = create_image(ColorSpace::Luma);
    single_tests(&u8_im, &u16_im, &f32_im);
}

#[test]
fn test_luma_a_to_other_colors() {
    let [u8_im, u16_im, f32_im] = create_image(ColorSpace::LumaA);
    single_tests(&u8_im, &u16_im, &f32_im);
}
