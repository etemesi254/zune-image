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

fn generic_bench<S: AsRef<Path>>(c: &mut Criterion, path: S, name: &str) {
    let data = read(path).unwrap();

    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("zune-jpeg", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())))
    });

    group.bench_function("mozjpeg", |b| {
        b.iter(|| black_box(decode_jpeg_mozjpeg(data.as_slice())))
    });
}

fn decode_no_samp(c: &mut Criterion) {
    generic_bench(
        c,
        sample_path().join("test-images/jpeg/benchmarks/speed_bench.jpg"),
        "jpeg: No sampling Baseline decode",
    );
}

fn decode_h_samp(c: &mut Criterion) {
    generic_bench(
        c,
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_horizontal_subsampling.jpg"),
        "jpeg: Horizontal Sub Sampling",
    );
}

fn decode_v_samp(c: &mut Criterion) {
    generic_bench(
        c,
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_vertical_subsampling.jpg"),
        "jpeg: Vertical sub sampling",
    );
}

fn decode_hv_samp(c: &mut Criterion) {
    generic_bench(
        c,
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_hv_subsampling.jpg"),
        "jpeg: HV sampling",
    );
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
    generic_bench(
        c,
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog.jpg"),
        "jpeg: No sampling Progressive decoding",
    );
}

fn decode_h_samp_prog(c: &mut Criterion) {
    generic_bench(
        c,
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog_h_sampling.jpg"),
        "jpeg: Progressive Horizontal Sub Sampling",
    )
}

fn decode_v_samp_prog(c: &mut Criterion) {
    generic_bench(
        c,
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog_v_sampling.jpg"),
        "jpeg: Progressive Vertical sub sampling",
    )
}

fn decode_hv_samp_prog(c: &mut Criterion) {
    generic_bench(
        c,
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog_hv_sampling.jpg"),
        "jpeg: Progressive HV sampling",
    )
}

fn decode_jpeg_opts(buf: &[u8], options: DecoderOptions) -> Vec<u8> {
    let mut d = JpegDecoder::new_with_options(ZCursor::new(buf), options);

    d.decode().unwrap()
}

fn decode_jpeg_incremental_mode(buf: &[u8]) -> Vec<u8> {
    let mut d = JpegDecoder::new(ZCursor::new(buf));
    d.set_incremental_mode(true);

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
use std::path::Path;
use std::rc::Rc;

/// Byte-slice cursor with an externally-adjustable visibility limit.
///
/// Reads/seeks beyond `limit` behave as if the data ended there (EOF). The
/// bench grows `limit` via the shared `Rc<Cell<usize>>` to simulate more
/// data arriving on the same decoder.
struct GrowableCursor<'a> {
    data: &'a [u8],
    position: usize,
    limit: Rc<Cell<usize>>,
}

impl<'a> GrowableCursor<'a> {
    fn new(data: &'a [u8], limit: Rc<Cell<usize>>) -> Self {
        Self {
            data,
            position: 0,
            limit,
        }
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
            SeekFrom::End(p) => self.visible() as i64 + p,
        };
        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }
}

/// One-shot decode of a DRI-bearing JPEG. Exercises the per-RST checkpoint
/// capture code path on every restart-interval boundary in steady state.
fn decode_restart_full(c: &mut Criterion) {
    let data = read(sample_path().join("test-images/jpeg/four_components.jpg")).unwrap();
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
    let data = read(sample_path().join("test-images/jpeg/four_components.jpg")).unwrap();
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
                Err(e) => assert!(e.is_recoverable_eof(), "got: {e:?}"),
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

fn decode_streaming_mode(c: &mut Criterion) {
    let data = read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_hv_subsampling.jpg"))
        .unwrap();
    let mut group = c.benchmark_group("jpeg: Incremental mode checkpoints");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("one-shot default", |b| {
        b.iter(|| black_box(decode_jpeg(data.as_slice())));
    });

    group.bench_function("one-shot incremental-mode", |b| {
        b.iter(|| black_box(decode_jpeg_incremental_mode(data.as_slice())));
    });

    let partial = data.len() * 80 / 100;

    group.bench_function("first retry default", |b| {
        b.iter(|| {
            let limit = Rc::new(Cell::new(partial));
            let cursor = GrowableCursor::new(data.as_slice(), Rc::clone(&limit));
            let mut decoder = JpegDecoder::new(cursor);
            decoder.decode_headers().expect("headers should fit in 80%");
            let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];

            match decoder.decode_into(&mut out) {
                Ok(()) => panic!("scan should not complete at 80% visibility"),
                Err(e) => assert!(e.is_recoverable_eof(), "got: {e:?}"),
            }

            limit.set(data.len());
            decoder
                .decode_into(&mut out)
                .expect("scan should complete with full data");
            black_box(out);
        });
    });

    group.bench_function("first retry incremental-mode", |b| {
        b.iter(|| {
            let limit = Rc::new(Cell::new(partial));
            let cursor = GrowableCursor::new(data.as_slice(), Rc::clone(&limit));
            let mut decoder = JpegDecoder::new(cursor);
            decoder.set_incremental_mode(true);
            decoder.decode_headers().expect("headers should fit in 80%");
            let mut out = vec![0u8; decoder.output_buffer_size().unwrap()];

            match decoder.decode_into(&mut out) {
                Ok(()) => panic!("scan should not complete at 80% visibility"),
                Err(e) => assert!(e.is_recoverable_eof(), "got: {e:?}"),
            }

            limit.set(data.len());
            decoder
                .decode_into(&mut out)
                .expect("scan should complete with full data");
            black_box(out);
        });
    });
}

#[derive(Clone, Copy)]
struct ProgressiveScanRange {
    data_start: usize,
    data_end:   usize,
    spec_start: u8,
    spec_end:   u8,
    succ_high:  u8
}

fn jpeg_markers(data: &[u8]) -> Vec<(usize, u8, Option<usize>)> {
    // Keep this standalone benchmark helper aligned with the equivalent
    // fixture scanner in zune-jpeg/tests/incremental.rs.
    let mut markers = Vec::new();
    let mut offset = 0;
    while offset + 1 < data.len() {
        if data[offset] != 0xFF {
            offset += 1;
            continue;
        }
        let code = data[offset + 1];
        if code == 0xFF || code == 0x00 {
            offset += 1;
            continue;
        }
        if code == 0xD8 || code == 0xD9 || (0xD0..=0xD7).contains(&code) {
            markers.push((offset, code, None));
            offset += 2;
            continue;
        }
        if offset + 3 >= data.len() {
            break;
        }
        let length = usize::from(u16::from_be_bytes([data[offset + 2], data[offset + 3]]));
        if length < 2 || offset + 2 + length > data.len() {
            break;
        }
        markers.push((offset, code, Some(length)));
        offset += 2 + length;
    }
    markers
}

fn progressive_scan_ranges(data: &[u8]) -> Vec<ProgressiveScanRange> {
    let markers = jpeg_markers(data);
    markers
        .iter()
        .enumerate()
        .filter_map(|(index, (offset, code, length))| {
            if *code != 0xDA {
                return None;
            }
            let length = length.expect("SOS must have a length");
            let payload = offset + 4;
            let components = usize::from(data[payload]);
            let params = payload + 1 + 2 * components;
            let data_start = offset + 2 + length;
            let data_end = markers
                .iter()
                .skip(index + 1)
                .find_map(|(next_offset, next_code, _)| {
                    (!((0xD0..=0xD7).contains(next_code))).then_some(*next_offset)
                })
                .unwrap_or(data.len());
            let successive = data[params + 2];
            Some(ProgressiveScanRange {
                data_start,
                data_end,
                spec_start: data[params],
                spec_end: data[params + 1],
                succ_high: successive >> 4
            })
        })
        .collect()
}

fn repeated_limits(start: usize, end: usize) -> [usize; 3] {
    assert!(end > start + 3, "scan range is too small for repeated retries");
    let length = end - start;
    [start + length / 4, start + length / 2, start + length * 3 / 4]
}

fn decode_with_repeated_growth(data: &[u8], limits: &[usize]) -> Vec<u8> {
    let first_limit = *limits.first().expect("at least one retry limit is required");
    let limit = Rc::new(Cell::new(first_limit));
    let cursor = GrowableCursor::new(data, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new(cursor);
    decoder.set_incremental_mode(true);
    decoder.decode_headers().expect("headers must fit before the first retry limit");
    let mut output = vec![0; decoder.output_buffer_size().unwrap()];

    for visible in limits {
        limit.set(*visible);
        let error = decoder
            .decode_into(&mut output)
            .expect_err("each partial visibility limit must stop at recoverable EOF");
        assert!(error.is_recoverable_eof(), "got: {error:?}");
    }

    limit.set(data.len());
    decoder
        .decode_into(&mut output)
        .expect("full input must complete after repeated retries");
    output
}

/// Measures total wall-clock work across three EOF/retry cycles plus the final
/// successful decode. This zune-only group measures retry work; the progressive
/// decode groups above provide one-shot zune-jpeg/mozjpeg comparisons.
fn decode_repeated_retry_cost(c: &mut Criterion) {
    let baseline = read(
        sample_path().join("test-images/jpeg/benchmarks/speed_bench_hv_subsampling.jpg")
    )
    .unwrap();
    let baseline_start = baseline
        .windows(2)
        .position(|window| window == [0xFF, 0xDA])
        .expect("baseline fixture must contain SOS");
    let baseline_sos_length = usize::from(u16::from_be_bytes([
        baseline[baseline_start + 2],
        baseline[baseline_start + 3]
    ]));
    let baseline_entropy_start = baseline_start + 2 + baseline_sos_length;
    let baseline_limits = repeated_limits(baseline_entropy_start, baseline.len() - 2);

    let progressive = read(sample_path().join("test-images/jpeg/benchmarks/speed_bench_prog.jpg"))
        .unwrap();
    let scans = progressive_scan_ranges(&progressive);
    let dc_first = scans
        .iter()
        .find(|scan| scan.spec_start == 0 && scan.spec_end == 0 && scan.succ_high == 0)
        .expect("progressive fixture must contain a first-DC scan");
    let ac_refinement = scans
        .iter()
        .find(|scan| scan.spec_start > 0 && scan.succ_high > 0)
        .expect("progressive fixture must contain an AC-refinement scan");
    let dc_limits = repeated_limits(dc_first.data_start, dc_first.data_end);
    let ac_refinement_limits =
        repeated_limits(ac_refinement.data_start, ac_refinement.data_end);

    let mut group = c.benchmark_group("jpeg: Repeated incremental retry cost");
    group.bench_function("baseline row checkpoints", |b| {
        b.iter(|| black_box(decode_with_repeated_growth(&baseline, &baseline_limits)))
    });
    group.bench_function("progressive first-DC MCU checkpoints", |b| {
        b.iter(|| black_box(decode_with_repeated_growth(&progressive, &dc_limits)))
    });
    group.bench_function("progressive AC-refinement MCU checkpoints", |b| {
        b.iter(|| {
            black_box(decode_with_repeated_growth(
                &progressive,
                &ac_refinement_limits
            ))
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
    decode_no_samp_opts,
    decode_restart_full,decode_restart_resume,decode_streaming_mode,
    decode_repeated_retry_cost);

criterion_main!(benches);
