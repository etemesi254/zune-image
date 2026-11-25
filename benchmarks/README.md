# Zune benchmarks

### Benchmark primitives.

This crate exposes the ability to compare `zune-` family of crates to other crates that
exists which do the same functionality

It does not contain any library specific code, most of the code exists
in the `benches` directory

Benchmarking is done via `criterion` 

## JPEG CI reports

`.github/workflows/criterion_benchmarks.yml` explicitly runs
`cargo bench --locked -p zune-benches --bench decode_jpeg -- zune-jpeg`.
This explicitly selects eleven one-shot zune-jpeg cases: baseline/progressive
sampling, grayscale and intrinsics/scalar decoding. Auxiliary DRI/retry/row
benchmarks and mozjpeg controls are not part of this workload. In particular,
the existing DRI retry harness assumes that an 80%-visible permissive decode
cannot complete; that assertion is not valid at the current base. It must not
be confused with a failure of benchmark reporting or suppressed with `tee`.

`decode_jpeg` is the Criterion target; `decode_jpeg_colorspaces` is a standalone
timing program and
`decode_jpeg_allocations` is an allocation-assertion program. Neither produces
Criterion baselines, so they are not inputs to this report.

The target reads all eight tracked `test-images/jpeg/benchmarks/*.jpg` fixtures
and `test-images/jpeg/four_components.jpg`, even when a case filter is used.
The runner checks their existence and identical hashes in both checkouts.
Mozjpeg needs NASM and a C build toolchain; the package's other development
dependencies also build, including spng/libz (CI installs `zlib1g-dev`, CMake and
pkg-config). The JPEG target does not link libvips or enable optional libheif.
No benchmark dependency or decoder MSRV change is needed.

For pull requests, the exact event base SHA and actual head SHA (not GitHub's
synthetic merge commit or the current `dev` tip) are checked out separately.
Both use Rust 1.89.0, the same recorded flags and separate Cargo target/result
directories, without build/result caches. Non-PR events run both variants at
the selected SHA to check the pipeline itself. `base` and `head` named
Criterion baselines are exported by pinned `critcmp 0.1.8`; `cargo criterion`
event-stream JSON is not a critcmp export.

Empty/malformed exports, nonpositive/nonfinite timings, zero JPEG cases,
different case sets and failed/empty comparisons fail the job. All measured
differences are reported without a regression threshold. Fork PRs use only
read permissions and publish job summaries/artifacts, not PR comments.
Artifacts are uploaded even on failure: exports, raw Criterion files,
comparison, validation/error logs, exact SHAs, fixture hashes, lockfiles and
toolchain/flags. Build binaries are excluded.
If a benchmark fails after producing some measurements, its raw results and
best-effort partial export are retained, but the job still fails.

To reproduce with two isolated checkouts and a fresh report directory:

```shell
cargo install critcmp --version 0.1.8 --locked
python3 -m unittest discover -s .github/scripts -p 'test_*.py' -v
RUSTUP_TOOLCHAIN=1.89.0 bash .github/scripts/jpeg-benchmark.sh \
  /path/to/base-checkout /path/to/head-checkout ./jpeg-report BASE_SHA HEAD_SHA
```

The default case filter is `zune-jpeg`. Sampling is 20 samples, 1 second warmup,
2 seconds measurement and 10,000 resamples per case. For a short pipeline smoke test, set
`JPEG_BENCH_FILTER='jpeg: HV sampling/zune-jpeg'`, `JPEG_SAMPLE_SIZE=10`,
`JPEG_WARM_UP_TIME=0.05`, `JPEG_MEASUREMENT_TIME=0.05` and
`JPEG_NRESAMPLES=1000`. These short samples verify reporting, not performance.
`JPEG_RUSTFLAGS` defaults to `-C target-cpu=native`; all effective settings are
recorded in `measurement.txt`. Do not overlap smoke measurements with other
performance timings.

## Formatting Criterion results

Criterion's JSON results can be converted into grouped Markdown tables like the
ones below. After running the benchmarks, use:

```shell
python3 benchmarks/format_criterion.py > benchmark-results.md
```

The default input is `target/criterion`. To compare saved runs from two systems:

```shell
python3 benchmarks/format_criterion.py \
  --run macOS=results/macos/criterion \
  --run Linux=results/linux/criterion \
  --output benchmark-results.md
```

The formatter uses Criterion's mean estimate, automatically selects readable
time units, calculates throughput from Criterion's byte or element metadata,
marks the fastest library in each benchmark as the winner, reports how many
times slower each alternative is, and uses `—` where a run has no matching
result.

## Running on MacOs

Tested on mac-os solana


1. Install brew
2. Install libvips
```shell
brew install libvips,nasm
```

   For the HEIF benchmark against libheif (`decode_heic`, `--features libheif`),
   also `brew install libheif`.
1. Create a symlink to allow the system linker to find libraries

```shell
sudo ln -s /opt/homebrew/lib /usr/local/lib
```

1. Run cargo bench

```shell
cargo bench --workspace
```

If you fail to symlink, the libraries

## On linux
This is a short one on ubuntu 

1. Install clang, libvips, libvips-dev , nasm (for mozjpeg)

```shell
sudo apt install clang libvips libvips-dev nasm
```

   For the HEIF benchmark against libheif (`decode_heic`, `--features libheif`),
   also `sudo apt install libheif-dev` (libheif 1.17 or newer).

1. Run the benchmark
```shell
cargo bench --workspace
```

Benchmarks

## On linux
System Specs
```text
Architecture:             x86_64
  CPU op-mode(s):         32-bit, 64-bit
  Address sizes:          40 bits physical, 48 bits virtual
  Byte Order:             Little Endian
CPU(s):                   4
  On-line CPU(s) list:    0-3
Vendor ID:                AuthenticAMD
  Model name:             AMD EPYC-Rome Processor
    CPU family:           23
    Model:                49
    Thread(s) per core:   1
    Core(s) per socket:   4
    Socket(s):            1
    Stepping:             0
    BogoMIPS:             4890.80
    Flags:                fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ht syscall nx mmxext fxsr_opt pdpe1gb rdtscp lm rep_good nopl cpuid extd_apicid tsc_known
                          _freq pni pclmulqdq ssse3 fma cx16 sse4_1 sse4_2 x2apic movbe popcnt aes xsave avx f16c rdrand hypervisor lahf_lm cmp_legacy cr8_legacy abm sse4a misalignsse 3dnowprefetch osvw topoext pe
                          rfctr_core ssbd ibrs ibpb stibp vmmcall fsgsbase bmi1 avx2 smep bmi2 rdseed adx smap clflushopt clwb sha_ni xsaveopt xsavec xgetbv1 clzero xsaveerptr wbnoinvd arat umip rdpid
Virtualization features:  
  Hypervisor vendor:      KVM
  Virtualization type:    full
Caches (sum of all):      
  L1d:                    128 KiB (4 instances)
  L1i:                    128 KiB (4 instances)
  L2:                     2 MiB (4 instances)
  L3:                     16 MiB (1 instance)

```

Benchmark results found in [Benchmark Results](./benchmark-results.md)