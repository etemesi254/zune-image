/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;
use std::io::{Cursor, Read};
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use zune_benches::sample_path;

fn decode_writer_flate(bytes: &[u8]) -> Vec<u8> {
    let mut writer = Vec::new();

    let mut deflater = flate2::read::ZlibDecoder::new(Cursor::new(bytes));

    deflater.read_to_end(&mut writer).unwrap();

    writer
}

fn decode_writer_zune(bytes: &[u8]) -> Vec<u8> {
    let options = zune_inflate::DeflateOptions::default().set_size_hint((1 << 20) * 50);

    let mut deflater = zune_inflate::DeflateDecoder::new_with_options(bytes, options);

    deflater.decode_zlib().unwrap()
}

fn decode_writer_libdeflate(bytes: &[u8]) -> Vec<u8> {
    let mut deflater = libdeflater::Decompressor::new();
    // decompressed size is 43 mb. so allocate 50 mb
    let mut out = vec![0; (1 << 20) * 50];

    deflater.zlib_decompress(bytes, &mut out).unwrap();
    out
}

fn decode_writer_flate_gz(bytes: &[u8]) -> Vec<u8> {
    let mut writer = Vec::new();

    let mut deflater = flate2::read::GzDecoder::new(Cursor::new(bytes));

    deflater.read_to_end(&mut writer).unwrap();

    writer
}

fn decode_writer_zune_gz(bytes: &[u8]) -> Vec<u8> {
    let options = zune_inflate::DeflateOptions::default().set_size_hint((1 << 20) * 50);

    let mut deflater = zune_inflate::DeflateDecoder::new_with_options(bytes, options);

    deflater.decode_gzip().unwrap()
}

fn decode_writer_libdeflate_gz(bytes: &[u8]) -> Vec<u8> {
    let mut deflater = libdeflater::Decompressor::new();
    // decompressed size is 43 mb. so allocate 50 mb
    let mut out = vec![0; (1 << 20) * 50];

    deflater.gzip_decompress(bytes, &mut out).unwrap();
    out
}

fn decode_test(c: &mut Criterion) {
    let path = sample_path().join("test-images/inflate/zlib/enwiki_part.zlib");

    let data = read(path).unwrap();

    let mut group = c.benchmark_group("inflate: enwiki zlib decoding");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("flate/zlib-ng", |b| {
        b.iter(|| black_box(decode_writer_flate(data.as_slice())))
    });

    group.bench_function("zune-inflate", |b| {
        b.iter(|| black_box(decode_writer_zune(data.as_slice())))
    });

    group.bench_function("libdeflate", |b| {
        b.iter(|| black_box(decode_writer_libdeflate(data.as_slice())))
    });
}

fn decode_test_crow(c: &mut Criterion) {
    let path = sample_path().join("test-images/inflate/zlib/png_artwork.zlib");

    let data = read(path).unwrap();

    let mut group = c.benchmark_group("inflate: zlib decoding-png zlib");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("flate/zlib-ng", |b| {
        b.iter(|| black_box(decode_writer_flate(data.as_slice())))
    });

    group.bench_function("zune-inflate", |b| {
        b.iter(|| black_box(decode_writer_zune(data.as_slice())))
    });

    group.bench_function("libdeflate", |b| {
        b.iter(|| black_box(decode_writer_libdeflate(data.as_slice())))
    });
}

fn decode_test_gzip(c: &mut Criterion) {
    let path = sample_path().join("test-images/inflate/gzip/tokio.tar.gz");
    let data = read(path).unwrap();

    let mut group = c.benchmark_group("inflate: gzip decoding, tokio-rs source code");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("flate/zlib-ng", |b| {
        b.iter(|| black_box(decode_writer_flate_gz(data.as_slice())))
    });

    group.bench_function("zune-inflate", |b| {
        b.iter(|| black_box(decode_writer_zune_gz(data.as_slice())))
    });

    group.bench_function("libdeflate", |b| {
        b.iter(|| black_box(decode_writer_libdeflate_gz(data.as_slice())))
    });
}

fn decode_test_gzip_json(c: &mut Criterion) {
    let path = sample_path().join("test-images/inflate/gzip/image.json.gz");
    let data = read(path).unwrap();

    let mut group = c.benchmark_group("inflate: gzip decoding, image-rs rustdoc json");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("flate/zlib-ng", |b| {
        b.iter(|| black_box(decode_writer_flate_gz(data.as_slice())))
    });

    group.bench_function("zune-inflate", |b| {
        b.iter(|| black_box(decode_writer_zune_gz(data.as_slice())))
    });

    group.bench_function("libdeflate", |b| {
        b.iter(|| black_box(decode_writer_libdeflate_gz(data.as_slice())))
    });
}
criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(20))
      };
    targets=decode_test_crow,decode_test,decode_test_gzip,decode_test_gzip_json);

criterion_main!(benches);
