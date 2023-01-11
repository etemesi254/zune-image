use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn decode_rapid_qoi(data: &[u8]) -> Vec<u8>
{
    rapid_qoi::Qoi::decode_alloc(data).unwrap().1
}

fn decode_zune_qoi(data: &[u8]) -> Vec<u8>
{
    zune_qoi::QoiDecoder::new(data).decode().unwrap()
}

fn bench_decode(c: &mut Criterion)
{
    let a = env!("CARGO_MANIFEST_DIR").to_string() + "/test_images/wikipedia_008.qoi";

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
