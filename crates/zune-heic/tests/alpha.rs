use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_heic::HeifDecoder;

fn sample(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/fuzz-samples/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn decoder(data: &[u8]) -> HeifDecoder<ZCursor<&[u8]>> {
    let options = DecoderOptions::default().hvec_set_use_videotoolbox(false);
    HeifDecoder::new_with_options(ZCursor::new(data), options)
}

#[test]
fn alpha_plane_is_decoded() {
    for (name, premultiplied) in [("alpha.heic", false), ("alpha-premultiplied.heic", true)] {
        let data = sample(name);
        let mut d = decoder(&data);
        d.decode_headers().unwrap();
        assert_eq!(d.colorspace(), Some(ColorSpace::RGBA), "{name}");
        assert_eq!(d.is_alpha_premultiplied(), premultiplied, "{name}");

        let pixels = d.decode().unwrap();
        assert_eq!(pixels.len(), d.width().unwrap() * d.height().unwrap() * 4);
        // the alpha channel carries real values, not just 255
        let alpha: Vec<u8> = pixels.iter().skip(3).step_by(4).copied().collect();
        assert!(alpha.iter().any(|&a| a < 128), "{name}: no transparent pixels");
        assert!(alpha.iter().any(|&a| a > 128), "{name}: no opaque pixels");
    }
}

#[test]
fn no_alpha_is_rgb() {
    let data = sample("sample1.heif");
    let mut d = decoder(&data);
    d.decode_headers().unwrap();
    assert_eq!(d.colorspace(), Some(ColorSpace::RGB));
    assert!(!d.is_alpha_premultiplied());
}
