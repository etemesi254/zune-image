use std::fs::read;
use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use zune_benches::sample_path;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

fn decode_zune_raw(buf: &[u8]) -> Vec<Vec<u8>> {
    let mut decoder = JpegDecoder::new(ZCursor::new(buf));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let count = raw.num_components().unwrap();
    let strides: Vec<usize> = layout[..count]
        .iter()
        .map(|plane| plane.width + 13)
        .collect();
    let mut storage: Vec<Vec<u8>> = layout[..count]
        .iter()
        .enumerate()
        .map(|(index, plane)| vec![0; strides[index] * plane.height])
        .collect();
    let mut planes: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes_strided(&mut planes, &strides)
        .unwrap();
    storage
}

fn decode_libjpeg_turbo_yuv(buf: &[u8]) -> turbojpeg::YuvImage<Vec<u8>> {
    turbojpeg::decompress_to_yuv(buf).unwrap()
}

fn compare_progressive_raw_output(c: &mut Criterion) {
    let mut group = c.benchmark_group("jpeg: libjpeg-turbo raw comparison");

    for (sampling, fixture) in [
        ("4:4:4", "speed_bench_prog.jpg"),
        ("4:2:2", "speed_bench_prog_h_sampling.jpg"),
        ("4:2:0", "speed_bench_prog_420.jpg")
    ] {
        let data = read(
            sample_path()
                .join("test-images/jpeg/benchmarks")
                .join(fixture)
        )
        .unwrap();
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("zune raw planes", sampling),
            &data,
            |b, input| b.iter(|| black_box(decode_zune_raw(input.as_slice())))
        );
        group.bench_with_input(
            BenchmarkId::new("libjpeg-turbo YUV planes", sampling),
            &data,
            |b, input| b.iter(|| black_box(decode_libjpeg_turbo_yuv(input.as_slice())))
        );
    }
}

criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(20))
      };
    targets=compare_progressive_raw_output);

criterion_main!(benches);
