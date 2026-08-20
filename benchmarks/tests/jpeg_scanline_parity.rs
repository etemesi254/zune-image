use std::fs::read;

use zune_benches::sample_path;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::{JpegDecoder, ScanlineReadStatus, ScanlineStatus};

#[derive(Clone, Copy, Debug)]
enum ReferenceColor {
    Rgb,
    Luma
}

fn decode_zune_scanlines(data: &[u8], color: ReferenceColor) -> Vec<u8> {
    let output = match color {
        ReferenceColor::Rgb => ColorSpace::RGB,
        ReferenceColor::Luma => ColorSpace::Luma
    };
    let options = DecoderOptions::default().jpeg_set_out_colorspace(output);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let mut scanlines = decoder.scanline_output();
    assert_eq!(scanlines.start().unwrap(), ScanlineStatus::Ready);
    let row_bytes = scanlines.output_row_bytes().unwrap();
    let height = scanlines.output_height().unwrap();
    let mut output = vec![0; row_bytes * height];

    while scanlines.output_scanline() < height {
        let row = scanlines.output_scanline();
        match scanlines
            .read_scanlines(&mut output[row * row_bytes..], row_bytes)
            .unwrap()
        {
            ScanlineReadStatus::RowsProcessed { rows } => assert!(rows > 0),
            status => panic!("unexpected scanline status {status:?}")
        }
    }
    assert_eq!(scanlines.finish().unwrap(), ScanlineStatus::Complete);
    output
}

fn decode_libjpeg(data: &[u8], color: ReferenceColor) -> Vec<u8> {
    let decoder = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS)
        .from_mem(data)
        .unwrap();
    match color {
        ReferenceColor::Rgb => {
            let mut image = decoder.rgb().unwrap();
            let rows: Vec<[u8; 3]> = image.read_scanlines().unwrap();
            image.finish().unwrap();
            rows.into_iter().flatten().collect()
        }
        ReferenceColor::Luma => {
            let mut image = decoder.grayscale().unwrap();
            let rows: Vec<[u8; 1]> = image.read_scanlines().unwrap();
            image.finish().unwrap();
            rows.into_iter().flatten().collect()
        }
    }
}

#[test]
fn converted_scanlines_match_libjpeg_across_sampling_modes() {
    for (name, fixture) in [
        ("none", "speed_bench.jpg"),
        ("horizontal", "speed_bench_horizontal_subsampling.jpg"),
        ("vertical", "speed_bench_vertical_subsampling.jpg"),
        ("hv", "speed_bench_hv_subsampling.jpg")
    ] {
        let data = read(
            sample_path()
                .join("test-images/jpeg/benchmarks")
                .join(fixture)
        )
        .unwrap();
        for color in [ReferenceColor::Rgb, ReferenceColor::Luma] {
            let actual = decode_zune_scanlines(&data, color);
            let reference = decode_libjpeg(&data, color);
            assert_eq!(actual.len(), reference.len(), "{name}/{color:?}");

            let mut max_delta = 0;
            let mut total_delta = 0_u64;
            for (&actual, &reference) in actual.iter().zip(&reference) {
                let delta = actual.abs_diff(reference);
                max_delta = max_delta.max(delta);
                total_delta += u64::from(delta);
            }
            let average_delta = total_delta as f64 / actual.len() as f64;
            eprintln!("{name}/{color:?}: max delta {max_delta}, average delta {average_delta}");
            assert!(
                max_delta <= 4,
                "{name}/{color:?}: max channel delta {max_delta} exceeds 4"
            );
            assert!(
                average_delta < 0.1,
                "{name}/{color:?}: average channel delta {average_delta} exceeds 0.1"
            );
        }
    }
}
