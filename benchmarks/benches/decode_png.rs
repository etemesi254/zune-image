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
use spng::DecodeFlags;
use zune_benches::sample_path;
use zune_jpeg::zune_core::bytestream::ZCursor;

fn decode_ref(data: &[u8]) -> Vec<u8> {
    let mut decoder = png::Decoder::new(data);
    decoder.set_transformations(png::Transformations::EXPAND);

    let mut reader = decoder.read_info().unwrap();

    // Allocate the output buffer.
    let mut buf = vec![0; reader.output_buffer_size()];
    // Read the next frame. An APNG might contain multiple frames.
    let _ = reader.next_frame(&mut buf).unwrap();

    buf
}

fn decode_zune(data: &[u8]) -> Vec<u8> {
    zune_png::PngDecoder::new(ZCursor::new(data))
        .decode_raw()
        .unwrap()
}

fn decode_spng(data: &[u8]) -> Vec<u8> {
    let cursor = std::io::Cursor::new(data);
    let mut decoder = spng::Decoder::new(cursor);
    decoder.set_decode_flags(DecodeFlags::TRANSPARENCY);

    let (_, mut reader) = decoder.read_info().unwrap();
    let output_buffer_size = reader.output_buffer_size();
    let mut out = vec![0; output_buffer_size];
    reader.next_frame(&mut out).unwrap();
    out
}

fn decode_lodepng(data: &[u8]) -> lodepng::Image {
    lodepng::Decoder::new().decode(data).unwrap()
}

fn decode_test(c: &mut Criterion) {
    let path = sample_path().join("test-images/png/benchmarks/speed_bench.png");
    let data = read(path).unwrap();

    let mut group = c.benchmark_group("png: PNG decoding baseline");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-png", |b| {
        b.iter(|| black_box(decode_zune(data.as_slice())))
    });

    group.bench_function("image-rs/png", |b| {
        b.iter(|| black_box(decode_ref(data.as_slice())))
    });

    group.bench_function("spng", |b| {
        b.iter(|| black_box(decode_spng(data.as_slice())))
    });

    group.bench_function("lodepng", |b| {
        b.iter(|| black_box(decode_lodepng(data.as_slice())))
    });
}

fn decode_test_interlaced(c: &mut Criterion) {
    let path = sample_path().join("test-images/png/benchmarks/speed_bench_interlaced.png");

    let data = read(path).unwrap();

    let mut group = c.benchmark_group("png: PNG decoding interlaced 8bpp");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-png", |b| {
        b.iter(|| black_box(decode_zune(data.as_slice())))
    });

    group.bench_function("image-rs/png", |b| {
        b.iter(|| black_box(decode_ref(data.as_slice())))
    });

    group.bench_function("spng", |b| {
        b.iter(|| black_box(decode_spng(data.as_slice())))
    });

    group.bench_function("lodepng", |b| {
        b.iter(|| black_box(decode_lodepng(data.as_slice())))
    });
}

fn decode_test_16_bit(c: &mut Criterion) {
    let path = sample_path().join("test-images/png/benchmarks/speed_bench_16.png");
    let data = read(path).unwrap();

    let mut group = c.benchmark_group("png: PNG decoding  16 bpp");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-png", |b| {
        b.iter(|| black_box(decode_zune(data.as_slice())))
    });

    group.bench_function("image-rs/png", |b| {
        b.iter(|| black_box(decode_ref(data.as_slice())))
    });

    group.bench_function("spng", |b| {
        b.iter(|| black_box(decode_spng(data.as_slice())))
    });

    group.bench_function("lodepng", |b| {
        b.iter(|| black_box(decode_lodepng(data.as_slice())))
    });
}

fn decode_test_trns_chunk(c: &mut Criterion) {
    let path = sample_path().join("test-images/png/benchmarks/speed_bench_palette.png");
    let data = read(path).unwrap();

    let mut group = c.benchmark_group("png: PNG decoding palette image");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-png", |b| {
        b.iter(|| black_box(decode_zune(data.as_slice())))
    });

    group.bench_function("image-rs/png", |b| {
        b.iter(|| black_box(decode_ref(data.as_slice())))
    });

    group.bench_function("spng", |b| {
        b.iter(|| black_box(decode_spng(data.as_slice())))
    });

    group.bench_function("lodepng", |b| {
        b.iter(|| black_box(decode_lodepng(data.as_slice())))
    });
}
criterion_group!(name=benches;
  config={
  let c = Criterion::default();
    c.measurement_time(Duration::from_secs(20))
  };
targets=decode_test_trns_chunk,decode_test_16_bit,decode_test,decode_test_interlaced
);

criterion_main!(benches);
