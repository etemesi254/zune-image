# Zune benchmarks

### Benchmark primitives.

This crate exposes the ability to compare `zune-` family of crates to other crates that
exists which do the same functionality

It does not contain any library specific code, most of the code exists
in the `benches` directory

Benchmarking is done via `criterion` 

## Running on MacOs

Tested on mac-os solana


1. Install brew
2. Install libvips
```shell
brew install libvips
```
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

## HDR Decoding

| Benchmark    | Library | Time     | Throughput |
|--------------|---------|----------|------------|
| File IO      | hdr     | 8.333 ms | —          |
| In-memory IO | hdr     | 6.606 ms | —          |

## Inflate / Zlib Decoding

| Benchmark                    | Library      | Time      | Throughput  |
|------------------------------|--------------|-----------|-------------|
| PNG zlib                     | zlib-ng      | 131.92 ms | 89.4 MiB/s  |
| PNG zlib                     | zune-inflate | 87.06 ms  | 135.5 MiB/s |
| PNG zlib                     | libdeflate   | 72.17 ms  | 163.5 MiB/s |
|                              |
| enwiki zlib                  | zlib-ng      | 144.62 ms | 92.1 MiB/s  |
| enwiki zlib                  | zune-inflate | 107.42 ms | 124.0 MiB/s |
| enwiki zlib                  | libdeflate   | 74.94 ms  | 177.7 MiB/s |
|                              |
| gzip (tokio-rs source)       | zlib-ng      | 56.24 ms  | 226.1 MiB/s |
| gzip (tokio-rs source)       | zune-inflate | 57.46 ms  | 221.2 MiB/s |
| gzip (tokio-rs source)       | libdeflate   | 38.52 ms  | 330.1 MiB/s |
|                              |
| gzip (image-rs rustdoc json) | zlib-ng      | 4.555 ms  | 88.0 MiB/s  |
| gzip (image-rs rustdoc json) | zune-inflate | 6.572 ms  | 61.0 MiB/s  |
| gzip (image-rs rustdoc json) | libdeflate   | 3.318 ms  | 120.8 MiB/s |

## JPEG Decoding

| Benchmark                           | Library                   | Time      | Throughput  |
|-------------------------------------|---------------------------|-----------|-------------|
| No sampling baseline                | zune-jpeg                 | 114.73 ms | 10.59 MiB/s |
| No sampling baseline                | mozjpeg                   | 119.90 ms | 10.14 MiB/s |
|                                     |
| Horizontal sub sampling             | zune-jpeg                 | 104.63 ms | 10.24 MiB/s |
| Horizontal sub sampling             | mozjpeg                   | 101.82 ms | 10.52 MiB/s |
|                                     |
| Vertical sub sampling               | zune-jpeg                 | 105.82 ms | 10.07 MiB/s |
| Vertical sub sampling               | mozjpeg                   | 147.96 ms | 7.21 MiB/s  |
|                                     |
| HV sampling                         | zune-jpeg                 | 99.38 ms  | 9.83 MiB/s  |
| HV sampling                         | mozjpeg                   | 95.52 ms  | 10.23 MiB/s |
|                                     |
| Grayscale                           | zune-jpeg                 | 57.40 ms  | 21.17 MiB/s |
| Grayscale                           | mozjpeg                   | 49.12 ms  | 24.74 MiB/s |
|                                     |
| Progressive HV sampling             | zune-jpeg                 | 356.36 ms | 3.22 MiB/s  |
| Progressive HV sampling             | mozjpeg                   | 312.63 ms | 3.67 MiB/s  |
|                                     |
| Progressive horizontal sub sampling | zune-jpeg                 | 354.89 ms | 3.26 MiB/s  |
| Progressive horizontal sub sampling | mozjpeg                   | 276.01 ms | 4.19 MiB/s  |
|                                     |
| No sampling progressive             | zune-jpeg                 | 423.85 ms | 3.12 MiB/s  |
| No sampling progressive             | mozjpeg                   | 354.97 ms | 3.73 MiB/s  |
|                                     |
| Progressive vertical sub sampling   | zune-jpeg                 | 356.78 ms | 3.21 MiB/s  |
| Progressive vertical sub sampling   | mozjpeg                   | 318.82 ms | 3.59 MiB/s  |
|                                     |
| Intrinsics                          | zune-jpeg (intrinsics)    | 116.86 ms | 10.40 MiB/s |
| Intrinsics                          | zune-jpeg (no intrinsics) | 112.25 ms | 10.83 MiB/s |
|                                     |
| DRI / Restart markers               | one-shot                  | 6.469 ms  | 21.08 MiB/s |
| DRI / Restart markers               | incremental resume        | 11.74 ms  | 11.62 MiB/s |

## PNG Decoding

| Benchmark       | Library      | Time      | Throughput  |
|-----------------|--------------|-----------|-------------|
| Palette image   | zune-png     | 106.53 ms | 48.93 MiB/s |
| Palette image   | image-rs/png | 92.84 ms  | 56.15 MiB/s |
| Palette image   | spng         | 85.12 ms  | 61.24 MiB/s |
|                 |
| 16 bpp          | zune-png     | 252.47 ms | 30.42 MiB/s |
| 16 bpp          | image-rs/png | 266.37 ms | 28.84 MiB/s |
| 16 bpp          | spng         | 1088.7 ms | 7.05 MiB/s  |
|                 |
| Baseline        | zune-png     | 226.61 ms | 26.44 MiB/s |
| Baseline        | image-rs/png | 201.41 ms | 29.75 MiB/s |
| Baseline        | spng         | 261.47 ms | 22.92 MiB/s |
|                 |
| Interlaced 8bpp | zune-png     | 268.57 ms | 37.56 MiB/s |
| Interlaced 8bpp | image-rs/png | 360.80 ms | 27.96 MiB/s |
| Interlaced 8bpp | spng         | 376.09 ms | 26.82 MiB/s |

## QOI Decoding

| Benchmark     | Library   | Time     | Throughput  |
|---------------|-----------|----------|-------------|
| Simple decode | rapid-qoi | 6.996 ms | 207.4 MiB/s |
| Simple decode | zune-qoi  | 16.36 ms | 88.7 MiB/s  |

## HEIF Decoding

| Benchmark   | Library              | Time      |
|-------------|----------------------|-----------|
| HEIF decode | zune-heif (software) | 255.47 ms |
| HEIF decode | heic                 | 387.77 ms |

## Image Processing

| Benchmark              | Library          | Time      | Throughput  |
|------------------------|------------------|-----------|-------------|
| Affine transform 45°   | libvips          | 434.57 ms | 2.80 MiB/s  |
| Affine transform 45°   | zune-image       | 368.68 ms | 3.30 MiB/s  |
|                        |
| Sobel                  | libvips          | 144.52 ms | 8.41 MiB/s  |
| Sobel                  | zune-image       | 427.62 ms | 2.84 MiB/s  |
|                        |
| Gamma                  | libvips          | 79.85 ms  | 15.22 MiB/s |
| Gamma                  | zune-image       | 86.24 ms  | 14.09 MiB/s |
|                        |
| Gaussian blur          | vips             | 109.92 ms | 11.06 MiB/s |
| Gaussian blur          | image-rs         | 816.86 ms | 1.49 MiB/s  |
| Gaussian blur          | zune-image       | 317.97 ms | 3.82 MiB/s  |
|                        |
| Premultiply            | libvips          | 288.84 ms | 4.21 MiB/s  |
| Premultiply            | zune-image       | 36.77 ms  | 33.05 MiB/s |
|                        |
| Rotate 90              | vips             | 78.99 ms  | 15.38 MiB/s |
| Rotate 90              | image-rs         | 183.57 ms | 6.62 MiB/s  |
| Rotate 90              | zune-image       | 163.34 ms | 7.44 MiB/s  |
| Rotate 180             | libvips          | 71.33 ms  | 17.04 MiB/s |
| Rotate 180             | zune-image       | 39.83 ms  | 30.51 MiB/s |
|                        |
| Invert                 | libvips          | 63.12 ms  | 19.25 MiB/s |
| Invert                 | zune-image       | 35.82 ms  | 33.93 MiB/s |
|                        |
| Resize linear kernel   | vips             | 41.81 ms  | 29.07 MiB/s |
| Resize linear kernel   | image-rs         | 1498.6 ms | 0.83 MiB/s  |
| Resize linear kernel   | zune-image       | 233.02 ms | 5.21 MiB/s  |
| Resize linear kernel   | fir              | 66.68 ms  | 18.22 MiB/s |
| Resize linear kernel   | stb-image-resize | 688.50 ms | 1.77 MiB/s  |
| Resize lanczos kernel  | vips             | 50.11 ms  | 24.25 MiB/s |
| Resize lanczos kernel  | image-rs         | 2142.5 ms | 0.58 MiB/s  |
| Resize lanczos kernel  | zune-image       | 275.84 ms | 4.41 MiB/s  |
| Resize lanczos kernel  | fir              | 77.18 ms  | 15.74 MiB/s |
| Resize lanczos kernel  | stb-image-resize | 690.96 ms | 1.76 MiB/s  |
|                        |
| Flip horizontal        | libvips          | 67.74 ms  | 17.94 MiB/s |
| Flip horizontal        | zune-image       | 41.06 ms  | 29.60 MiB/s |
| Flip vertical          | libvips          | 56.87 ms  | 21.37 MiB/s |
| Flip vertical          | zune-image       | 38.87 ms  | 31.27 MiB/s |
|                        |
| Resize mitchell kernel | vips             | 47.65 ms  | 25.50 MiB/s |
| Resize mitchell kernel | image-rs         | 2144.8 ms | 0.58 MiB/s  |
| Resize mitchell kernel | zune-image       | 293.26 ms | 4.14 MiB/s  |
| Resize mitchell kernel | fir              | 70.78 ms  | 17.17 MiB/s |
| Resize mitchell kernel | stb-image-resize | 690.55 ms | 1.76 MiB/s  |