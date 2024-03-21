/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Benchmarks for

use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use zune_benches::sample_path;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;
use zune_png::zune_core::bytestream::ZCursor;

fn decode_jpeg(buf: &[u8]) -> Vec<u8> {
    let mut d = JpegDecoder::new(ZCursor::new(buf));

    d.decode().unwrap()
}

fn decode_jpeg_mozjpeg(buf: &[u8]) -> Vec<[u8; 3]> {
    let p = std::panic::catch_unwind(|| {
        let d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS)
            .from_mem(buf)
            .unwrap();

        // rgba() enables conversion
        let mut image = d.rgb().unwrap();

        let pixels: Vec<[u8; 3]> = image.read_scanlines().unwrap();

        image.finish().unwrap();
        pixels
    })
    .unwrap();

    p
}

fn decode_jpeg_image_rs(buf: &[u8]) -> Vec<u8> {
    let mut decoder = jpeg_decoder::Decoder::new(buf);

    decoder.decode().unwrap()
}

fn decode_no_samp(c: &mut Criterion) {
    let a = sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg");

    let data = read(a).unwrap();
    let mut group = c.benchmark_group("jpeg: No sampling Baseline decode");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function("imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice())))
    });
}

fn decode_h_samp(c: &mut Criterion) {
    let data = read(
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_horizontal_subsampling.jpg")
    )
    .unwrap();
    let mut group = c.benchmark_group("jpeg: Horizontal Sub Sampling");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function("imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice())))
    });
}

fn decode_v_samp(c: &mut Criterion) {
    let data = read(
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_vertical_subsampling.jpg")
    )
    .unwrap();
    let mut group = c.benchmark_group("jpeg: Vertical sub sampling");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function("imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice())))
    });
}

fn decode_hv_samp(c: &mut Criterion) {
    let data =
        read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_hv_subsampling.jpg"))
            .unwrap();
    let mut group = c.benchmark_group("jpeg: HV sampling");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function("imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice())))
    });
}

fn decode_jpeg_grayscale(buf: &[u8]) -> Vec<u8> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);

    let mut d = JpegDecoder::new_with_options(ZCursor::new(buf), options);

    d.decode().unwrap()
}

fn decode_jpeg_mozjpeg_grayscale(buf: &[u8]) -> Vec<[u8; 1]> {
    let p = std::panic::catch_unwind(|| {
        let d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS)
            .from_mem(buf)
            .unwrap();

        // rgba() enables conversion
        let mut image = d.grayscale().unwrap();

        let pixels: Vec<[u8; 1]> = image.read_scanlines().unwrap();

        image.finish().unwrap();

        pixels
    })
    .unwrap();

    p
}

fn criterion_benchmark_grayscale(c: &mut Criterion) {
    let a = sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg");

    let data = read(a).unwrap();

    let mut group = c.benchmark_group("jpeg: Grayscale decoding");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg_grayscale(data.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg_grayscale(data.as_slice())))
    });
}

fn decode_no_samp_prog(c: &mut Criterion) {
    let a = sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog.jpg");
    let data = read(a).unwrap();
    let mut group = c.benchmark_group("jpeg: No sampling Progressive decoding");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function("imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice())))
    });
}

fn decode_h_samp_prog(c: &mut Criterion) {
    let x = read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog_h_sampling.jpg"))
        .unwrap();
    let mut group = c.benchmark_group("jpeg: Progressive Horizontal Sub Sampling");
    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(x.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(x.as_slice())))
    });

    group.bench_function("imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(x.as_slice())))
    });
}

fn decode_v_samp_prog(c: &mut Criterion) {
    let x = read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog_v_sampling.jpg"))
        .unwrap();

    let mut group = c.benchmark_group("jpeg: Progressive Vertical sub sampling");

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(x.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(x.as_slice())))
    });

    group.bench_function("imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(x.as_slice())))
    });
}

fn decode_hv_samp_prog(c: &mut Criterion) {
    let x =
        read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog_hv_sampling.jpg"))
            .unwrap();
    let mut group = c.benchmark_group("jpeg: Progressive HV sampling");
    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(x.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(x.as_slice())))
    });

    group.bench_function("imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(x.as_slice())))
    });
}

fn decode_jpeg_opts(buf: &[u8], options: DecoderOptions) -> Vec<u8> {
    let mut d = JpegDecoder::new_with_options(ZCursor::new(buf), options);

    d.decode().unwrap()
}

fn decode_no_samp_opts(c: &mut Criterion) {
    let a = sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg");

    let data = read(a).unwrap();
    let mut group = c.benchmark_group("jpeg: zune-jpeg Intrinsics");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("intrinsics", |b| {
        b.iter(|| {
            let opt = DecoderOptions::default();
            black_box(decode_jpeg_opts(data.as_slice(), opt));
        })
    });
    group.bench_function("no intrinsics", |b| {
        b.iter(|| {
            let opt = DecoderOptions::default().set_use_unsafe(false);
            black_box(decode_jpeg_opts(data.as_slice(), opt));
        })
    });
}

criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(20))
      };
    targets=decode_no_samp,decode_h_samp,decode_v_samp,
    decode_hv_samp,criterion_benchmark_grayscale,
    decode_hv_samp_prog,decode_h_samp_prog,decode_no_samp_prog,decode_v_samp_prog,
    decode_no_samp_opts);

criterion_main!(benches);
