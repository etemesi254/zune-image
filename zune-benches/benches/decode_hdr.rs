/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use image::ImageFormat;
use zune_benches::sample_path;
use zune_png::zune_core::options::DecoderOptions;

fn zune_decode_hdr(buf: &[u8]) -> zune_image::image::Image {
    zune_image::image::Image::read(buf, DecoderOptions::new_fast()).unwrap()
}

fn image_decode_hdr(buf: &[u8]) -> image::DynamicImage {
    image::load_from_memory_with_format(buf, ImageFormat::Hdr).unwrap()
}

fn bench_decode_memorial(c: &mut Criterion) {
    let a = sample_path().join("test-images/hdr/memorial.hdr");

    let data = read(a).unwrap();
    let mut group = c.benchmark_group("hdr: Simple decode(memorial-hdr)");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("image-rs/hdr", |b| {
        b.iter(|| black_box(image_decode_hdr(data.as_slice())))
    });

    group.bench_function("zune-image/hdr", |b| {
        b.iter(|| black_box(zune_decode_hdr(data.as_slice())))
    });
}

fn _bench_decode_sample(c: &mut Criterion) {
    // BUG: sample format not supported by image, it doesn't recoginse hdr magic bytes
    let a = sample_path().join("test-images/hdr/sample_640×426.hdr");

    let data = read(a).unwrap();
    let mut group = c.benchmark_group("hdr: Simple decode (sample_hdr)");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("image-rs/hdr", |b| {
        b.iter(|| black_box(image_decode_hdr(data.as_slice())))
    });

    group.bench_function("zune-image/hdr", |b| {
        b.iter(|| black_box(zune_decode_hdr(data.as_slice())))
    });
}

criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(20))
      };
    targets=bench_decode_memorial);

criterion_main!(benches);
