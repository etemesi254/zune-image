/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Benchmarks for

use std::fs::read;
use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
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

// ---------------------------------------------------------------------------
// Restart-marker / resumability path
//
// The benchmarks above all use JPEGs without restart markers (DRI), so the
// per-RST checkpoint code added for incremental decoding is never exercised
// by them. The benchmarks below close that gap with a DRI-bearing input
// (four_components.jpg, restart interval = 7 MCUs, 4-component baseline).
// They cover:
//   * one-shot decode of a DRI image (steady-state RST checkpoint cost),
//   * an incremental decode that triggers a recoverable-EOF retry, so the
//     RST checkpoint capture + resume seek path is measured end-to-end.
// ---------------------------------------------------------------------------

use std::cell::Cell;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::rc::Rc;

/// Byte-slice cursor with an externally-adjustable visibility limit.
///
/// Reads/seeks beyond `limit` behave as if the data ended there (EOF). The
/// bench grows `limit` via the shared `Rc<Cell<usize>>` to simulate more
/// data arriving on the same decoder.
struct GrowableCursor<'a> {
    data:     &'a [u8],
    position: usize,
    limit:    Rc<Cell<usize>>
}

impl<'a> GrowableCursor<'a> {
    fn new(data: &'a [u8], limit: Rc<Cell<usize>>) -> Self {
        Self { data, position: 0, limit }
    }

    fn visible(&self) -> usize {
        self.limit.get().min(self.data.len())
    }
}

impl Read for GrowableCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let visible = self.visible();
        if self.position >= visible {
            return Ok(0);
        }
        let available = &self.data[self.position..visible];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.position += n;
        Ok(n)
    }
}

impl BufRead for GrowableCursor<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let visible = self.visible();
        if self.position >= visible {
            return Ok(&[]);
        }
        Ok(&self.data[self.position..visible])
    }

    fn consume(&mut self, amt: usize) {
        self.position += amt;
    }
}

impl Seek for GrowableCursor<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::Current(p) => self.position as i64 + p,
            SeekFrom::End(p) => self.visible() as i64 + p
        };
        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start"
            ));
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }
}

/// One-shot decode of a DRI-bearing JPEG. Exercises the per-RST checkpoint
/// capture code path on every restart-interval boundary in steady state.
fn decode_restart_full(c: &mut Criterion) {
    let data =
        read(sample_path().join("test-images/jpeg/four_components.jpg")).unwrap();
    let mut group = c.benchmark_group("jpeg: DRI / Restart markers");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("one-shot", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())));
    });
}

/// Incremental decode of a DRI-bearing JPEG that triggers a recoverable EOF
/// and then resumes. The decoder is freshly constructed each iteration, so
/// the measurement reflects: one full header parse, one truncated scan that
/// returns ExhaustedData near the end, then a final scan that resumes from
/// the latest RST checkpoint and finishes the decode.
fn decode_restart_resume(c: &mut Criterion) {
    let data =
        read(sample_path().join("test-images/jpeg/four_components.jpg")).unwrap();
    let mut group = c.benchmark_group("jpeg: DRI / Restart markers");
    group.throughput(Throughput::Bytes(data.len() as u64));

    // Expose ~80% of the file on the first attempt so the scan loop hits
    // recoverable EOF after several RST checkpoints have been recorded.
    let partial = data.len() * 80 / 100;

    group.bench_function("incremental resume", |b| {
        b.iter(|| {
            let limit = Rc::new(Cell::new(partial));
            let cursor = GrowableCursor::new(data.as_slice(), Rc::clone(&limit));
            let mut decoder = JpegDecoder::new(cursor);
            decoder.decode_headers().expect("headers should fit in 80%");
            let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];

            // First scan attempt: expect recoverable EOF.
            match decoder.decode_into(&mut out) {
                Ok(()) => panic!("scan should not complete at 80% visibility"),
                Err(e) => assert!(e.is_recoverable_eof(), "got: {e:?}")
            }

            // Expose remaining bytes and resume from the latest checkpoint.
            limit.set(data.len());
            decoder
                .decode_into(&mut out)
                .expect("scan should complete with full data");
            black_box(out);
        });
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
    decode_no_samp_opts,
    decode_restart_full,decode_restart_resume);

criterion_main!(benches);
