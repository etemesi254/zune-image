//! Benchmarks for grayscale decoding

use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use zune_core::colorspace::ColorSpace;
use zune_jpeg::{JpegDecoder, ZuneJpegOptions};

fn decode_jpeg(buf: &[u8]) -> Vec<u8>
{
    let mut d = JpegDecoder::new_with_options(
        ZuneJpegOptions::new().set_out_colorspace(ColorSpace::Luma),
        buf
    );

    d.decode().unwrap()
}

fn decode_jpeg_mozjpeg(buf: &[u8]) -> Vec<[u8; 1]>
{
    let p = std::panic::catch_unwind(|| {
        let d = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS)
            .from_mem(buf)
            .unwrap();

        // rgba() enables conversion
        let mut image = d.grayscale().unwrap();

        let pixels: Vec<[u8; 1]> = image.read_scanlines().unwrap();

        assert!(image.finish_decompress());

        pixels
    })
    .unwrap();

    p
}

fn criterion_benchmark(c: &mut Criterion)
{
    let a = env!("CARGO_MANIFEST_DIR").to_string() + "/benches/images/speed_bench.jpg";

    let data = read(a).unwrap();

    let mut group = c.benchmark_group("[jpeg]: Grayscale decoding");

    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });
}

criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(5))
      };
    targets=criterion_benchmark);

criterion_main!(benches);
