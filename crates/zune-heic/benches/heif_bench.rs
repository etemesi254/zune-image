use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use heic::{DecoderConfig, PixelLayout};
use libheif_rs::{Channel, ColorSpace, HeifContext, ItemId, LibHeif, Result, RgbChroma};
use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_heic::HeifDecoder;
fn bench_heif_decoding(c: &mut Criterion) {
    // Read the file once into memory so we are benchmarking CPU decoding speed,
    // not disk I/O performance.
    let image_bytes = std::fs::read("fuzz-samples/IMG_4862.HEIC")
        .expect("Failed to read test.heic. Please ensure the file exists.");

    let mut group = c.benchmark_group("HEIF Decoding");
    group.throughput(Throughput::Bytes(image_bytes.len() as u64));

    // -- zune-heif Benchmark --
    group.bench_function("heic: zune-heif (software decoder)", |b| {
        b.iter(|| {
            let data = ZCursor::new(black_box(image_bytes.as_slice()));
            let options = DecoderOptions::new_fast().hvec_set_use_videotoolbox(false);
            let mut decoder = HeifDecoder::new_with_options(data, options);

            decoder.decode_headers().unwrap();
            let _colorspace = decoder.colorspace().unwrap();

            // Decode the actual pixel data and black_box the result
            let decoded_data = decoder.decode().unwrap();
            black_box(decoded_data);
        });
    });
    #[cfg(target_os = "macos")]
    {
        group.bench_function("heic: zune-heif (hardware decoder)", |b| {
            b.iter(|| {
                let data = ZCursor::new(black_box(image_bytes.as_slice()));
                let options = DecoderOptions::new_fast().hvec_set_use_videotoolbox(true);

                let mut decoder = HeifDecoder::new_with_options(data, options);

                decoder.decode_headers().unwrap();
                let _colorspace = decoder.colorspace().unwrap();

                // Decode the actual pixel data and black_box the result
                let decoded_data = decoder.decode().unwrap();
                black_box(decoded_data);
            });
        });
    }

    group.bench_function("heic: libheif", |b| {
        b.iter(|| {
            let lib_heif = LibHeif::new();
            let ctx = HeifContext::read_from_bytes(image_bytes.as_slice()).unwrap();
            let handle = ctx.primary_image_handle().unwrap();

            // Decode the image
            let image = lib_heif.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None).unwrap();

            // Get "pixels"
            let planes = image.planes();
            let interleaved_plane = planes.interleaved.unwrap();
            black_box(interleaved_plane);
        })
    });
    // -- libheif-rs Benchmark --
    group.bench_function("heic", |b| {
        b.iter(|| {
            let output = DecoderConfig::new()
                .decode(&image_bytes, PixelLayout::Rgb8)
                .unwrap();

            black_box(output.data);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_heif_decoding);
criterion_main!(benches);
