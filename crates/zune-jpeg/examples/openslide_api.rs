use std::fs;
use std::io::Cursor;

use zune_core::colorspace::ColorSpace;
use zune_core::options::{DecoderOptions, InputColorspaceOverride, JpegScale};
use zune_jpeg::{
    assemble_split_jpeg, DecodeRegion, JpegDecoder, JpegDimensions, RegionDecodeMode,
};

/// Demonstrate the OpenSlide-oriented JPEG helpers on local tile fixture files.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jpeg = fs::read("tile.jpg")?;

    let mut full = JpegDecoder::new(Cursor::new(jpeg.as_slice()));
    let pixels = full.decode()?;
    let info = full.info().expect("headers were decoded");
    println!("decoded {}x{} RGB bytes={}", info.width, info.height, pixels.len());

    let region = DecodeRegion {
        x: 128,
        y: 96,
        width: 256,
        height: 256,
    };
    let mut region_decoder = JpegDecoder::new(Cursor::new(jpeg.as_slice()));
    let region_pixels = region_decoder.decode_region(region, RegionDecodeMode::BestEffort)?;
    println!("region RGB bytes={}", region_pixels.len());

    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::BGRA)
        .jpeg_set_input_colorspace_override(InputColorspaceOverride::Force(ColorSpace::YCbCr))
        .jpeg_set_scale(JpegScale::Half);
    let mut bgra = JpegDecoder::new_with_options(Cursor::new(jpeg.as_slice()), options);
    let bgra_pixels = bgra.decode_region(region, RegionDecodeMode::BestEffort)?;
    println!("scaled BGRA region bytes={}", bgra_pixels.len());

    let table_header = fs::read("jpeg_tables.bin")?;
    let abbreviated_scan = fs::read("tile_scan.bin")?;
    let mut assembled = Vec::new();
    assemble_split_jpeg(
        &table_header,
        &abbreviated_scan,
        2,
        JpegDimensions {
            width: info.width,
            height: info.height,
        },
        &mut assembled,
    )?;
    println!("assembled split JPEG bytes={}", assembled.len());

    Ok(())
}
