//! Benchmarks for

use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use zune_jpeg::JpegDecoder;

fn decode_jpeg(buf: &[u8]) -> Vec<u8>
{
    let mut d = JpegDecoder::new(buf);

    d.decode_buffer().unwrap()
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
    let a = env!("CARGO_MANIFEST_DIR").to_string() + "/benches/images/speed_bench.jpg";

    let data = read(a).unwrap();
    let mut group = c.benchmark_group("No sampling");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("Baseline JPEG Decoding zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("Baseline JPEG Decoding  mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function("Baseline JPEG Decoding  imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice())))
    });
}
fn decode_h_samp(c: &mut Criterion)
{
    let data = read(
        env!("CARGO_MANIFEST_DIR").to_string()
            + "/benches/images/speed_bench_horizontal_subsampling.jpg",
    )
    .unwrap();
    let mut group = c.benchmark_group("Horizontal Sub Sampling");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("Horizontal sampling JPEG Decoding zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("Horizontal sampling JPEG Decoding  mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function(
        "Horizontal sampling JPEG Decoding  imagers/jpeg-decoder",
        |b| b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice()))),
    );
}

fn decode_v_samp(c: &mut Criterion)
{
    let data = read(
        env!("CARGO_MANIFEST_DIR").to_string()
            + "/benches/images/speed_bench_vertical_subsampling.jpg",
    )
    .unwrap();
    let mut group = c.benchmark_group("Vertical sub sampling");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("Vertical sub-sampling JPEG Decoding zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("Vertical sub-sampling JPEG Decoding  mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function(
        "Vertical sub-sampling sampling JPEG Decoding  imagers/jpeg-decoder",
        |b| b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice()))),
    );
}

fn decode_hv_samp(c: &mut Criterion)
{
    let data = read(
        env!("CARGO_MANIFEST_DIR").to_string() + "/benches/images/speed_bench_hv_subsampling.jpg",
    )
    .unwrap();
    let mut group = c.benchmark_group("HV sampling");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("HV sampling JPEG Decoding zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("HV sampling JPEG Decoding  mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });

    group.bench_function("HV sampling JPEG Decoding  imagers/jpeg-decoder", |b| {
        b.iter(|| black_box(decode_jpeg_image_rs(data.as_slice())))
    });
}
criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(20))
      };
    targets=decode_no_samp,decode_h_samp,decode_v_samp,decode_hv_samp);

criterion_main!(benches);
