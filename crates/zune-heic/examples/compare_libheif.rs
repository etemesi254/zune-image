//! Check that zune-heic decodes an image bit-identically to libheif.
//!
//! ```text
//! heif-convert image.heic reference.png
//! cargo run --release --example compare_libheif -- image.heic reference.png
//! ```
//!
//! Exits with status 1 if any sample differs. Used by the
//! `HEIC conformance` CI workflow.
use std::process::ExitCode;

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_heic::HeifDecoder;
use zune_png::PngDecoder;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let [_, heic_path, png_path] = args.as_slice() else {
        eprintln!("usage: compare_libheif <image.heic> <libheif-output.png>");
        return ExitCode::from(2);
    };
    let heic = std::fs::read(heic_path).expect("cannot read the HEIC file");
    let png = std::fs::read(png_path).expect("cannot read the PNG file");

    // software decoding only, so this tests zune-heic and not the OS decoder
    let options = DecoderOptions::default().hvec_set_use_videotoolbox(false);
    let mut decoder = HeifDecoder::new_with_options(ZCursor::new(heic.as_slice()), options);
    let ours = decoder.decode().expect("zune-heic failed to decode");
    let (width, height) = (decoder.width().unwrap(), decoder.height().unwrap());
    let channels = decoder.colorspace().unwrap().num_components();

    let mut png_decoder = PngDecoder::new(ZCursor::new(png.as_slice()));
    let theirs = png_decoder.decode_raw().expect("cannot decode the PNG file");
    let (png_w, png_h) = png_decoder.dimensions().unwrap();
    let png_channels = png_decoder.colorspace().unwrap().num_components();

    let name = std::path::Path::new(heic_path).file_name().unwrap().to_string_lossy();
    if (width, height, channels) != (png_w, png_h, png_channels) {
        println!(
            "FAIL {name}: zune-heic gives {width}x{height}x{channels}, libheif {png_w}x{png_h}x{png_channels}"
        );
        return ExitCode::FAILURE;
    }

    let mut differing = 0usize;
    let mut max_diff = 0u8;
    let mut first = None;
    for (i, (a, b)) in ours.chunks_exact(channels).zip(theirs.chunks_exact(channels)).enumerate() {
        if a != b {
            differing += 1;
            let d = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
            max_diff = max_diff.max(d);
            first.get_or_insert((i % width, i / width, a.to_vec(), b.to_vec()));
        }
    }
    match first {
        None => {
            println!("OK   {name}: {width}x{height}, identical to libheif");
            ExitCode::SUCCESS
        }
        Some((x, y, a, b)) => {
            println!(
                "FAIL {name}: {differing} of {} pixels differ (max {max_diff}); first at ({x}, {y}): zune-heic {a:?}, libheif {b:?}",
                width * height
            );
            ExitCode::FAILURE
        }
    }
}
