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
3. Create a symlink to allow the system linker to find libraries

```shell
sudo ln -s /opt/homebrew/lib /usr/local/lib
```

4. Run cargo bench

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

2. Run the benchmark
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

### Image decoding

#### HDR IO

| Group   | Benchmark     | Time (median) | Notes                     |
| ------- | ------------- | ------------: | ------------------------- |
| HDR I/O | File I/O      |       8.61 ms | 3% outliers               |
| HDR I/O | In-memory I/O |       6.72 ms | ~22% faster than file I/O |


#### Inflate ZLIB decoding
| Dataset      | Backend      | Time (median) |   Throughput | Relative   |
| ------------ | ------------ | ------------: | -----------: | ---------- |
| PNG zlib     | libdeflate   |      73.04 ms | 161.52 MiB/s | Fastest  |
| PNG zlib     | zune-inflate |      90.32 ms | 130.61 MiB/s | +24% slower |
| PNG zlib     | zlib-ng      |     136.37 ms |  86.50 MiB/s | +87% slower |
| Enwiki zlib  | libdeflate   |      74.24 ms | 179.39 MiB/s |  Fastest |
| Enwiki zlib  | zune-inflate |      96.55 ms | 137.93 MiB/s | +30% slower |
| Enwiki zlib  | zlib-ng      |     148.08 ms |  89.93 MiB/s | +99% slower |
| Tokio gzip   | libdeflate   |      38.44 ms | 330.75 MiB/s |  Fastest |
| Tokio gzip   | zlib-ng      |      55.60 ms | 228.64 MiB/s | +45% slower |
| Tokio gzip   | zune-inflate |      57.23 ms | 222.13 MiB/s | +49% slower |
| Rustdoc gzip | libdeflate   |       3.30 ms | 121.32 MiB/s |  Fastest |
| Rustdoc gzip | zlib-ng      |       4.59 ms |  87.28 MiB/s | +39% slower |
| Rustdoc gzip | zune-inflate |       6.02 ms |  66.60 MiB/s | +82% slower |

#### JPEG Decoding 

| Scenario                | zune-jpeg |   mozjpeg | Winner    |
| ----------------------- | --------: | --------: | --------- |
| Baseline decode         | 114.39 ms | 113.91 ms | Tie       |
| Horizontal subsampling  | 103.71 ms | 102.48 ms | mozjpeg   |
| Vertical subsampling    | 108.63 ms | 152.56 ms | zune-jpeg |
| HV sampling             |  99.36 ms |  94.34 ms | mozjpeg   |
| Grayscale               |  58.62 ms |  48.73 ms | mozjpeg   |
| Progressive HV          | 356.41 ms | 323.81 ms | mozjpeg   |
| Progressive horizontal  | 358.05 ms | 278.72 ms | mozjpeg   |
| Progressive no sampling | 440.21 ms | 364.52 ms | mozjpeg   |
| Progressive vertical    | 364.25 ms | 324.55 ms | mozjpeg   |

#### PNG Decoding
| Scenario        |  zune-png | image-rs/png |      spng | Winner   |
| --------------- | --------: | -----------: | --------: | -------- |
| Palette image   |  89.37 ms |     92.69 ms |  95.97 ms | zune-png |
| 16 bpp          | 515.83 ms |    268.30 ms |   1.179 s | image-rs |
| Baseline        | 234.72 ms |    200.04 ms | 298.10 ms | image-rs |
| Interlaced 8bpp | 315.74 ms |    362.45 ms | 397.79 ms | zune-png |


#### QOI Decoding
| Decoder   |     Time |   Throughput | Relative      |
| --------- | -------: | -----------: | ------------- |
| rapid-qoi |  7.00 ms | 207.32 MiB/s | 🥇 Fastest    |
| zune-qoi  | 16.42 ms |  88.36 MiB/s | ~2.35× slower |

### Image processing

### Affine Transform

| Backend    | Time (median) | Throughput |
| ---------- | ------------: | ---------: |
| libvips    |     420.96 ms | 2.89 MiB/s |
| zune-image |     393.30 ms | 3.09 MiB/s |

#### Sobel
| Backend    | Time (median) | Throughput |
| ---------- | ------------: | ---------: |
| libvips    |     147.46 ms | 8.24 MiB/s |
| zune-image |     278.61 ms | 4.36 MiB/s |

#### Gamma Correction
| Backend    | Time (median) |  Throughput |
| ---------- | ------------: | ----------: |
| libvips    |      78.43 ms | 15.49 MiB/s |
| zune-image |      96.78 ms | 12.56 MiB/s |

#### Gaussian Blur
| Backend    | Time (median) |  Throughput |
| ---------- | ------------: | ----------: |
| libvips    |     107.90 ms | 11.26 MiB/s |
| zune-image |     327.04 ms |  3.72 MiB/s |
| image-rs   |     773.22 ms |  1.57 MiB/s |

#### Premultiply Alpha

| Backend    | Time (median) |  Throughput |
| ---------- | ------------: | ----------: |
| libvips    |     285.60 ms |  4.25 MiB/s |
| zune-image |      70.80 ms | 17.17 MiB/s |

#### Rotate 90

| Backend    | Time (median) |  Throughput |
| ---------- | ------------: | ----------: |
| libvips    |     102.55 ms | 11.85 MiB/s |
| zune-image |     128.09 ms |  9.49 MiB/s |
| image-rs   |     211.46 ms |  5.75 MiB/s |

#### Rotate 180
| Backend    | Time (median) |  Throughput |
| ---------- | ------------: | ----------: |
| libvips    |      85.94 ms | 14.14 MiB/s |
| zune-image |      79.81 ms | 15.23 MiB/s |

#### Invert 
| Backend    | Time (median) |  Throughput |
| ---------- | ------------: | ----------: |
| libvips    |      77.30 ms | 15.72 MiB/s |
| zune-image |      76.81 ms | 15.82 MiB/s |

#### Flip horizontal
| Backend    | Time (median) |  Throughput |
| ---------- | ------------: | ----------: |
| libvips    |      83.05 ms | 14.63 MiB/s |
| zune-image |      80.38 ms | 15.12 MiB/s |

#### Flip Vertical
| Backend    | Time (median) |  Throughput |
| ---------- | ------------: | ----------: |
| libvips    |      72.56 ms | 16.75 MiB/s |
| zune-image |      80.51 ms | 15.09 MiB/s |

#### Resize Linear Kernel
| Backend          | Time (median) |  Throughput |
| ---------------- | ------------: | ----------: |
| libvips          |      41.65 ms | 29.18 MiB/s |
| fir              |      63.69 ms | 19.08 MiB/s |
| zune-image       |     209.86 ms |  5.79 MiB/s |
| stb-image-resize |     657.61 ms |  1.85 MiB/s |
| image-rs         |       1.370 s |  0.91 MiB/s |


#### Resize Lanczos Kernel
| Backend          | Time (median) |  Throughput |
| ---------------- | ------------: | ----------: |
| libvips          |      51.50 ms | 23.60 MiB/s |
| fir              |      73.19 ms | 16.60 MiB/s |
| zune-image       |     259.88 ms |  4.68 MiB/s |
| stb-image-resize |     653.83 ms |  1.86 MiB/s |
| image-rs         |       2.095 s |  0.59 MiB/s |


#### Resize - Mitchell Kernel 
| Backend          | Time (median) |  Throughput |
| ---------------- | ------------: | ----------: |
| libvips          |      49.02 ms | 24.79 MiB/s |
| fir              |      74.62 ms | 16.29 MiB/s |
| zune-image       |     255.53 ms |  4.76 MiB/s |
| stb-image-resize |     663.00 ms |  1.83 MiB/s |
| image-rs         |       2.124 s |  0.59 MiB/s |
