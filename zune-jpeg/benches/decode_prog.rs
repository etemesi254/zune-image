//! Benchmarks for

use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use zune_jpeg::JpegDecoder;

fn decode_jpeg(buf: &[u8]) -> Vec<u8>
{
    let mut d = JpegDecoder::new(buf);

    d.decode().unwrap()
}

fn decode_jpeg_mozjpeg(buf: &[u8]) -> Vec<[u8; 3]>
{
    let p = std::panic::catch_unwind(|| {
        let d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS)
            .from_mem(buf)
            .unwrap();

        // rgba() enables conversion
        let mut image = d.rgb().unwrap();

        let pixels: Vec<[u8; 3]> = image.read_scanlines().unwrap();

        assert!(image.finish_decompress());

        pixels
    })
    .unwrap();

    p
}

fn decode_jpeg_image_rs(buf: &[u8]) -> Vec<u8>
{
    let mut decoder = jpeg_decoder::Decoder::new(buf);

    decoder.decode().unwrap()
}

fn decode_no_samp(c: &mut Criterion)
{
    let a = env!("CARGO_MANIFEST_DIR").to_string() + "/benches/images/speed_bench_prog.jpg";

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
fn decode_h_samp(c: &mut Criterion)
{
    let x = read(
        env!("CARGO_MANIFEST_DIR").to_string() + "/benches/images/speed_bench_prog_h_sampling.jpg"
    )
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

fn decode_v_samp(c: &mut Criterion)
{
    let x = read(
        env!("CARGO_MANIFEST_DIR").to_string() + "/benches/images/speed_bench_prog_v_sampling.jpg"
    )
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

fn decode_hv_samp(c: &mut Criterion)
{
    let x = read(
        env!("CARGO_MANIFEST_DIR").to_string() + "/benches/images/speed_bench_prog_hv_sampling.jpg"
    )
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
criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(20))
      };
    targets=decode_no_samp,decode_h_samp,decode_v_samp,decode_hv_samp);

criterion_main!(benches);
