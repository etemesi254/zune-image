#!/bin/bash
# Benchmark script: compare decode performance between dev and current branch.
# Uses a simple Rust binary that measures wall-clock time over many iterations.
set -e

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
BENCH_BIN="target/release/bench_decode_perf"

cat > /tmp/bench_decode_perf.rs << 'EOF'
use std::fs::read;
use std::time::Instant;
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;

fn main() {
    let files = [
        "test-images/jpeg/benchmarks/speed_bench.jpg",
        "test-images/jpeg/benchmarks/speed_bench_horizontal_subsampling.jpg",
        "test-images/jpeg/benchmarks/speed_bench_vertical_subsampling.jpg",
        "test-images/jpeg/benchmarks/speed_bench_hv_subsampling.jpg",
        "test-images/jpeg/benchmarks/speed_bench_prog.jpg",
    ];

    let warmup = 20;
    let iterations = 200;

    for file in &files {
        let data = read(file).expect(file);
        let name = file.rsplit('/').next().unwrap();

        // warmup
        for _ in 0..warmup {
            let mut d = JpegDecoder::new(ZCursor::new(&data[..]));
            let _ = d.decode().unwrap();
        }

        let start = Instant::now();
        for _ in 0..iterations {
            let mut d = JpegDecoder::new(ZCursor::new(&data[..]));
            let _ = d.decode().unwrap();
        }
        let elapsed = start.elapsed();
        let per_iter_us = elapsed.as_micros() as f64 / iterations as f64;
        let throughput_mbs = (data.len() as f64 / 1_000_000.0) / (per_iter_us / 1_000_000.0);

        println!("{name:<55} {per_iter_us:>8.1} µs/iter  ({throughput_mbs:>6.1} MB/s)");
    }
}
EOF

echo "=== Building benchmark binary (release) ==="
# Build using a small helper crate that depends on zune-jpeg
mkdir -p /tmp/bench_crate/src
cat > /tmp/bench_crate/Cargo.toml << TOML
[package]
name = "bench_decode_perf"
version = "0.1.0"
edition = "2021"

[dependencies]
zune-jpeg = { path = "$(pwd)/crates/zune-jpeg" }

[[bin]]
name = "bench_decode_perf"
path = "src/main.rs"
TOML

cp /tmp/bench_decode_perf.rs /tmp/bench_crate/src/main.rs

echo ""
echo "=== Benchmarking: $CURRENT_BRANCH ==="
cargo build --release --manifest-path /tmp/bench_crate/Cargo.toml 2>/dev/null
/tmp/bench_crate/target/release/bench_decode_perf

echo ""
echo "=== Switching to dev branch ==="
git stash --quiet 2>/dev/null || true
git checkout dev --quiet

echo "=== Benchmarking: dev ==="
cargo build --release --manifest-path /tmp/bench_crate/Cargo.toml 2>/dev/null
/tmp/bench_crate/target/release/bench_decode_perf

echo ""
echo "=== Switching back to $CURRENT_BRANCH ==="
git checkout "$CURRENT_BRANCH" --quiet
git stash pop --quiet 2>/dev/null || true

echo ""
echo "Done. Compare the numbers above — they should be within noise (~1-2%)."
