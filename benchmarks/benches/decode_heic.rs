/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! HEIF/HEIC decoding: zune-heic against the `heic` crate and, with the
//! `libheif` feature (needs the system libheif), against libheif.
//!
//! ```text
//! cargo bench -p zune-benches --bench decode_heic
//! cargo bench -p zune-benches --bench decode_heic --features libheif
//! ```

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use heic::{DecoderConfig, PixelLayout};
use zune_benches::sample_path;
use zune_heic::zune_core::bytestream::ZCursor;
use zune_heic::zune_core::options::DecoderOptions;
use zune_heic::HeifDecoder;

fn decode_zune(data: &[u8], videotoolbox: bool) -> Vec<u8> {
    let options = DecoderOptions::new_fast().hvec_set_use_videotoolbox(videotoolbox);
    let mut decoder = HeifDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode().unwrap()
}

fn bench_heif_decoding(c: &mut Criterion) {
    // Read the file once into memory so we are benchmarking CPU decoding speed,
    // not disk I/O performance.
    let path = sample_path().join("crates/zune-heic/fuzz-samples/IMG_4862.HEIC");
    let image_bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let mut group = c.benchmark_group("HEIF Decoding");
    group.throughput(Throughput::Bytes(image_bytes.len() as u64));

    group.bench_function("heic: zune-heif (software decoder)", |b| {
        b.iter(|| black_box(decode_zune(black_box(&image_bytes), false)));
    });

    #[cfg(target_os = "macos")]
    group.bench_function("heic: zune-heif (hardware decoder)", |b| {
        b.iter(|| black_box(decode_zune(black_box(&image_bytes), true)));
    });

    #[cfg(feature = "libheif")]
    {
        use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};

        group.bench_function("heic: libheif", |b| {
            b.iter(|| {
                let lib_heif = LibHeif::new();
                let ctx = HeifContext::read_from_bytes(black_box(&image_bytes)).unwrap();
                let handle = ctx.primary_image_handle().unwrap();
                let image = lib_heif
                    .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)
                    .unwrap();
                // the interleaved RGB pixels, without copying them
                black_box(image.planes().interleaved.unwrap().data.len());
                black_box(image)
            });
        });
    }

    group.bench_function("heic", |b| {
        b.iter(|| {
            let output = DecoderConfig::new()
                .decode(black_box(&image_bytes), PixelLayout::Rgb8)
                .unwrap();
            black_box(output.data)
        });
    });

    group.finish();
}

criterion_group!(benches, bench_heif_decoding);
criterion_main!(benches);
