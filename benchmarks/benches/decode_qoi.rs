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
use zune_benches::sample_path;
use zune_jpeg::zune_core::bytestream::ZCursor;

fn decode_rapid_qoi(data: &[u8]) -> Vec<u8> {
    rapid_qoi::Qoi::decode_alloc(data).unwrap().1
}

fn decode_zune_qoi(data: &[u8]) -> Vec<u8> {
    zune_qoi::QoiDecoder::new(ZCursor::new(data))
        .decode()
        .unwrap()
}

fn bench_decode(c: &mut Criterion) {
    let a = sample_path().join("test-images/qoi/benches/wikipedia_008.qoi");

    let data = read(a).unwrap();
    let mut group = c.benchmark_group("qoi: Simple decode");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("rapid-qoi", |b| {
        b.iter(|| black_box(decode_rapid_qoi(data.as_slice())))
    });

    group.bench_function("zune-qoi", |b| {
        b.iter(|| black_box(decode_zune_qoi(data.as_slice())))
    });
}

criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(20))
      };
    targets=bench_decode);

criterion_main!(benches);
