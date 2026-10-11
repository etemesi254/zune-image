use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_heic::HeifDecoder;

fn decode(name: &str) -> (usize, usize, ColorSpace, Vec<u8>) {
    let data = std::fs::read(format!("{}/fuzz-samples/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let options = DecoderOptions::default().hvec_set_use_videotoolbox(false);
    let mut d = HeifDecoder::new_with_options(ZCursor::new(data.as_slice()), options);
    let pixels = d.decode().unwrap();
    (d.width().unwrap(), d.height().unwrap(), d.colorspace().unwrap(), pixels)
}

#[test]
fn clean_aperture_rotation_and_mirror() {
    // 301x203 coded as 302x204: cropped (`clap`), rotated and mirrored, with alpha
    let (w, h, cs, pixels) = decode("transforms-alpha.heic");
    assert_eq!((w, h, cs), (203, 301, ColorSpace::RGBA));
    assert_eq!(pixels.len(), w * h * 4);

    // 33x17 coded as 64x64: cropped, rotated and mirrored
    let (w, h, cs, pixels) = decode("transforms-odd.heic");
    assert_eq!((w, h, cs), (17, 33, ColorSpace::RGB));
    assert_eq!(pixels.len(), w * h * 3);
}

#[test]
fn greyscale_with_clean_aperture() {
    let (w, h, cs, pixels) = decode("grey.heic");
    assert_eq!((w, h, cs), (301, 203, ColorSpace::Luma));
    assert_eq!(pixels.len(), w * h);
}
