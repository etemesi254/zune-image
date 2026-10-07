//! RGBA output of four-component (CMYK and YCCK) images is the RGB output plus an opaque
//! alpha channel.

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

fn decode(data: &[u8], colorspace: ColorSpace) -> (Vec<u8>, Option<ColorSpace>) {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let pixels = decoder.decode().unwrap();
    (pixels, decoder.input_colorspace())
}

fn check(name: &str, data: &[u8], input: ColorSpace) {
    let (rgb, input_colorspace) = decode(data, ColorSpace::RGB);
    assert_eq!(input_colorspace, Some(input), "{name}");
    let (rgba, _) = decode(data, ColorSpace::RGBA);

    assert_eq!(rgba.len(), rgb.len() / 3 * 4, "{name}");
    for (i, (rgba, rgb)) in rgba.chunks_exact(4).zip(rgb.chunks_exact(3)).enumerate() {
        assert!(
            rgba[..3] == rgb[..] && rgba[3] == 255,
            "{name}: pixel {i} is {rgba:?} in RGBA, {rgb:?} in RGB"
        );
    }
}

#[test]
fn cmyk_rgba_is_rgb_with_opaque_alpha() {
    check(
        "cymk.jpg",
        include_bytes!("../../../test-images/jpeg/cymk.jpg"),
        ColorSpace::CMYK
    );
}

#[test]
fn ycck_rgba_is_rgb_with_opaque_alpha() {
    check(
        "four_components.jpg",
        include_bytes!("../../../test-images/jpeg/four_components.jpg"),
        ColorSpace::YCCK
    );
}
