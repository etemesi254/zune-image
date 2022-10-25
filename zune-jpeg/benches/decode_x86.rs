//! Benchmarks for

use std::fs::read;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use zune_jpeg::{JpegDecoder, ZuneJpegOptions};

fn decode_jpeg(buf: &[u8], options: ZuneJpegOptions) -> Vec<u8>
{
    let mut d = JpegDecoder::new_with_options(options, buf);

    d.decode_buffer().unwrap()
}

fn decode_no_samp(c: &mut Criterion)
{
    let a = env!("CARGO_MANIFEST_DIR").to_string() + "/benches/images/speed_bench.jpg";

    let data = read(a).unwrap();
    let mut group = c.benchmark_group("No sampling");
    group.bench_function("Baseline JPEG Decoding zune-jpeg Allowed intrinsics", |b| {
        b.iter(|| {
            let opt = ZuneJpegOptions::new();
            black_box(decode_jpeg(data.as_slice(), opt));
        })
    });
    group.bench_function("Baseline JPEG Decoding zune-jpeg no intrinsics", |b| {
        b.iter(|| {
            let opt = ZuneJpegOptions::new().set_use_unsafe(false);
            black_box(decode_jpeg(data.as_slice(), opt));
        })
    });
}
criterion_group!(name=benches;
      config={
      let c = Criterion::default();
        c.measurement_time(Duration::from_secs(20))
      };
    targets=decode_no_samp);

criterion_main!(benches);
