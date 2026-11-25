use std::error::Error;
use std::path::Path;

use turbojpeg::{Compressor, Image, PixelFormat, Subsamp};

fn main() -> Result<(), Box<dyn Error>> {
    let (width, height) = (33, 35);
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    std::fs::create_dir_all(&directory)?;
    for (name, subsamp, textured) in [
        ("flat_420.jpg", Subsamp::Sub2x2, false),
        ("texture_444.jpg", Subsamp::None, true),
        ("texture_422.jpg", Subsamp::Sub2x1, true),
        ("texture_420.jpg", Subsamp::Sub2x2, true)
    ] {
        let mut pixels = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                pixels.extend_from_slice(&if textured {
                    [
                        ((17 * x + 29 * y + x * y) % 256) as u8,
                        ((47 * x + 7 * y + 13) % 256) as u8,
                        ((11 * x + 43 * y + 19 * (x ^ y)) % 256) as u8
                    ]
                } else {
                    [128; 3]
                });
            }
        }
        let mut compressor = Compressor::new()?;
        compressor.set_quality(90)?;
        compressor.set_subsamp(subsamp)?;
        compressor.set_progressive(textured)?;
        let jpeg = compressor.compress_to_vec(Image {
            pixels: &pixels,
            width,
            pitch: width * 3,
            height,
            format: PixelFormat::RGB
        })?;
        let path = directory.join(name);
        std::fs::write(&path, &jpeg)?;
        println!("{}: {} bytes", path.display(), jpeg.len());
    }
    Ok(())
}
