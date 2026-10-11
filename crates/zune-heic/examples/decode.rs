//! Decode a HEIF/HEIC image and save it as a PPM/PGM/PAM file, which most
//! image viewers and tools (e.g. ImageMagick, GIMP) can open.
//!
//! ```text
//! cargo run --release --example decode -- image.heic out.ppm
//! ```
//!
//! Pass `--software` to use the software decoder on macOS as well.
use std::io::Write;
use std::time::Instant;

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_heic::HeifDecoder;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let software = args.iter().any(|a| a == "--software");
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [input, output] = paths.as_slice() else {
        eprintln!("usage: decode [--software] <image.heic> <output.ppm>");
        std::process::exit(2);
    };

    let data = std::fs::read(input).expect("cannot read the input file");
    let options = DecoderOptions::default().hvec_set_use_videotoolbox(!software);
    let mut decoder = HeifDecoder::new_with_options(ZCursor::new(data.as_slice()), options);

    decoder.decode_headers().expect("not a valid HEIF file");
    let (width, height) = (decoder.width().unwrap(), decoder.height().unwrap());
    let colorspace = decoder.colorspace().unwrap();
    println!("{input}: {width}x{height} {colorspace:?}");
    if let Some(exif) = decoder.exif_data() {
        println!("  Exif: {} bytes", exif.len());
    }
    if let Some(icc) = decoder.icc_data() {
        println!("  ICC profile: {} bytes", icc.len());
    }

    let start = Instant::now();
    let pixels = decoder.decode().expect("decoding failed");
    println!("  decoded in {:.1} ms", start.elapsed().as_secs_f64() * 1e3);

    // PGM for grey, PPM for RGB, PAM for anything with alpha
    let header = match colorspace {
        ColorSpace::Luma => format!("P5\n{width} {height}\n255\n"),
        ColorSpace::RGB => format!("P6\n{width} {height}\n255\n"),
        ColorSpace::LumaA => format!(
            "P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH 2\nMAXVAL 255\nTUPLTYPE GRAYSCALE_ALPHA\nENDHDR\n"
        ),
        ColorSpace::RGBA => format!(
            "P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n"
        ),
        other => panic!("cannot save {other:?} images"),
    };
    let mut file = std::io::BufWriter::new(std::fs::File::create(output).expect("cannot create the output file"));
    file.write_all(header.as_bytes()).unwrap();
    file.write_all(&pixels).unwrap();
    println!("  wrote {output}");
}
