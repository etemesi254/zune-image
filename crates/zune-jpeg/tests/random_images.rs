use std::fs::OpenOptions;
use std::io::Write;

use mozjpeg::ColorSpace as OutColorSpace;
use zune_core::colorspace::ColorSpace;
use zune_jpeg::{JpegDecoder, ZuneJpegOptions};

fn write_output(name: &str, pixels: &[u8], width: usize, height: usize, colorspace: OutColorSpace)
{
    let output: String = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/outputs/random/";
    std::fs::create_dir_all(&output).unwrap();

    std::panic::catch_unwind(|| {
        let mut comp = mozjpeg::Compress::new(colorspace);

        comp.set_size(width, height);
        comp.set_mem_dest();
        comp.start_compress();

        assert!(comp.write_scanlines(pixels));

        comp.finish_compress();

        let jpeg_bytes = comp.data_to_vec().unwrap();

        let mut v = OpenOptions::new()
            .write(true)
            .create(true)
            .open(output.clone() + "/" + name)
            .unwrap();

        v.write_all(&jpeg_bytes).unwrap();

        // write to file, etc.
    })
    .unwrap();
}

#[test]
fn huffman_third_index()
{
    //
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/huffman_third_index.jpg";
    let data = &std::fs::read(path).unwrap();

    let mut decoder = JpegDecoder::new_with_options(
        ZuneJpegOptions::default().set_out_colorspace(ColorSpace::Luma),
        data
    );
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");
    write_output(
        "huffman_third_index.jpg",
        &pixels,
        decoder.width() as usize,
        decoder.height() as usize,
        OutColorSpace::JCS_GRAYSCALE
    );
}

#[test]
fn single_qt()
{
    // This image has a single quantization header
    // with multiple QT tables defined.
    // Allows us to ensure that the multi-table QT handling logic works
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/single_qt.jpeg";
    let data = &std::fs::read(path).unwrap();

    let mut decoder = JpegDecoder::new_with_options(
        ZuneJpegOptions::default().set_out_colorspace(ColorSpace::Luma),
        data
    );
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");
    write_output(
        "single_qt.jpg",
        &pixels,
        decoder.width() as usize,
        decoder.height() as usize,
        OutColorSpace::JCS_GRAYSCALE
    );
}

#[test]
fn google_pixel()
{
    //
    let path = env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/google_pixel.jpg";

    let data = &std::fs::read(path).unwrap();

    let mut decoder = JpegDecoder::new_with_options(
        ZuneJpegOptions::default().set_out_colorspace(ColorSpace::Luma),
        data
    );
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");
    write_output(
        "google_pixel.jpg",
        &pixels,
        decoder.width() as usize,
        decoder.height() as usize,
        OutColorSpace::JCS_GRAYSCALE
    );
}

#[test]
fn google_pixel_progressive()
{
    //
    let path =
        env!("CARGO_MANIFEST_DIR").to_string() + "/tests/inputs/google_pixel_progressive.jpg";

    let data = &std::fs::read(path).unwrap();

    let mut decoder = JpegDecoder::new_with_options(
        ZuneJpegOptions::default().set_out_colorspace(ColorSpace::Luma),
        data
    );
    // Grayscale
    let pixels = decoder.decode().expect("Test failed decoding");
    write_output(
        "google_pixel_progressive.jpg",
        &pixels,
        decoder.width() as usize,
        decoder.height() as usize,
        OutColorSpace::JCS_GRAYSCALE
    );
}
