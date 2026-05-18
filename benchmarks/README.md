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

# Cross-Platform Benchmark Comparison (macOS vs. Linux)

## 1. HDR Decoding
| Benchmark    | Library | macOS Time | Linux Time | macOS Thrpt | Linux Thrpt |
|:-------------|:--------|:-----------|:-----------|:------------|:------------|
| File IO      | hdr     | 5.42 ms    | 8.33 ms    | —           | —           |
| In-memory IO | hdr     | 2.78 ms    | 6.61 ms    | —           | —           |

---

## 2. Inflate / Zlib Decoding
| Benchmark                | Library      | macOS Time | Linux Time | macOS Thrpt  | Linux Thrpt  |
|:-------------------------|:-------------|:-----------|:-----------|:-------------|:-------------|
| **PNG zlib**             | zlib-ng      | 107.88 ms  | 131.92 ms  | 109.35 MiB/s | 89.40 MiB/s  |
|                          | zune-inflate | 53.46 ms   | 87.06 ms   | 220.67 MiB/s | 135.50 MiB/s |
|                          | libdeflate   | 36.16 ms   | 72.17 ms   | 326.25 MiB/s | 163.50 MiB/s |
| **enwiki zlib**          | zlib-ng      | 122.53 ms  | 144.62 ms  | 108.69 MiB/s | 92.10 MiB/s  |
|                          | zune-inflate | 64.71 ms   | 107.42 ms  | 205.80 MiB/s | 124.00 MiB/s |
|                          | libdeflate   | 38.12 ms   | 74.94 ms   | 349.39 MiB/s | 177.70 MiB/s |
| **gzip (tokio source)**  | zlib-ng      | 36.28 ms   | 56.24 ms   | 350.46 MiB/s | 226.10 MiB/s |
|                          | zune-inflate | 35.93 ms   | 57.46 ms   | 353.86 MiB/s | 221.20 MiB/s |
|                          | libdeflate   | 21.58 ms   | 38.52 ms   | 589.18 MiB/s | 330.10 MiB/s |
| **gzip (image-rs json)** | zlib-ng      | 3.96 ms    | 4.56 ms    | 101.25 MiB/s | 88.00 MiB/s  |
|                          | zune-inflate | 3.77 ms    | 6.57 ms    | 106.23 MiB/s | 61.00 MiB/s  |
|                          | libdeflate   | 1.46 ms    | 3.32 ms    | 274.27 MiB/s | 120.80 MiB/s |

---

## 3. JPEG Decoding
| Benchmark                 | Library            | macOS Time | Linux Time | macOS Thrpt | Linux Thrpt |
|:--------------------------|:-------------------|:-----------|:-----------|:------------|:------------|
| **No sampling baseline**  | zune-jpeg          | 50.42 ms   | 114.73 ms  | 24.10 MiB/s | 10.59 MiB/s |
|                           | mozjpeg            | 49.97 ms   | 119.90 ms  | 24.32 MiB/s | 10.14 MiB/s |
| **Horizontal sampling**   | zune-jpeg          | 46.27 ms   | 104.63 ms  | 23.16 MiB/s | 10.24 MiB/s |
|                           | mozjpeg            | 41.44 ms   | 101.82 ms  | 25.86 MiB/s | 10.52 MiB/s |
| **Vertical sampling**     | zune-jpeg          | 50.27 ms   | 105.82 ms  | 21.21 MiB/s | 10.07 MiB/s |
|                           | mozjpeg            | 40.99 ms   | 147.96 ms  | 26.01 MiB/s | 7.21 MiB/s  |
| **HV sampling**           | zune-jpeg          | 43.96 ms   | 99.38 ms   | 22.23 MiB/s | 9.83 MiB/s  |
|                           | mozjpeg            | 36.44 ms   | 95.52 ms   | 26.82 MiB/s | 10.23 MiB/s |
| **Grayscale**             | zune-jpeg          | 34.19 ms   | 57.40 ms   | 35.54 MiB/s | 21.17 MiB/s |
|                           | mozjpeg            | 28.33 ms   | 49.12 ms   | 42.90 MiB/s | 24.74 MiB/s |
| **Prog. HV sampling**     | zune-jpeg          | 105.58 ms  | 356.36 ms  | 10.86 MiB/s | 3.22 MiB/s  |
|                           | mozjpeg            | 96.54 ms   | 312.63 ms  | 11.87 MiB/s | 3.67 MiB/s  |
| **Prog. Horiz sampling**  | zune-jpeg          | 108.21 ms  | 354.89 ms  | 10.69 MiB/s | 3.26 MiB/s  |
|                           | mozjpeg            | 100.85 ms  | 276.01 ms  | 11.47 MiB/s | 4.19 MiB/s  |
| **No sample progressive** | zune-jpeg          | 123.37 ms  | 423.85 ms  | 10.73 MiB/s | 3.12 MiB/s  |
|                           | mozjpeg            | 125.95 ms  | 354.97 ms  | 10.51 MiB/s | 3.73 MiB/s  |
| **Prog. Vert sampling**   | zune-jpeg          | 108.48 ms  | 356.78 ms  | 10.57 MiB/s | 3.21 MiB/s  |
|                           | mozjpeg            | 96.95 ms   | 318.82 ms  | 11.82 MiB/s | 3.59 MiB/s  |
| **Intrinsics test**       | zune-jpeg (with)   | 49.84 ms   | 116.86 ms  | 24.38 MiB/s | 10.40 MiB/s |
|                           | zune-jpeg (none)   | 49.56 ms   | 112.25 ms  | 24.52 MiB/s | 10.83 MiB/s |
| **DRI / Restart markers** | one-shot           | 3.19 ms    | 6.47 ms    | 42.76 MiB/s | 21.08 MiB/s |
|                           | incremental resume | 5.51 ms    | 11.74 ms   | 24.76 MiB/s | 11.62 MiB/s |

---

## 4. PNG Decoding
| Benchmark           | Library      | macOS Time | Linux Time | macOS Thrpt  | Linux Thrpt |
|:--------------------|:-------------|:-----------|:-----------|:-------------|:------------|
| **Palette image**   | zune-png     | 35.66 ms   | 106.53 ms  | 146.18 MiB/s | 48.93 MiB/s |
|                     | image-rs/png | 29.02 ms   | 92.84 ms   | 179.60 MiB/s | 56.15 MiB/s |
|                     | spng         | 54.44 ms   | 85.12 ms   | 95.75 MiB/s  | 61.24 MiB/s |
| **16 bpp**          | zune-png     | 186.45 ms  | 252.47 ms  | 41.20 MiB/s  | 30.42 MiB/s |
|                     | image-rs/png | 165.40 ms  | 266.37 ms  | 46.44 MiB/s  | 28.84 MiB/s |
|                     | spng         | 541.24 ms  | 1088.70 ms | 14.19 MiB/s  | 7.05 MiB/s  |
| **Baseline**        | zune-png     | 133.73 ms  | 226.61 ms  | 44.81 MiB/s  | 26.44 MiB/s |
|                     | image-rs/png | 149.37 ms  | 201.41 ms  | 40.12 MiB/s  | 29.75 MiB/s |
|                     | spng         | 301.03 ms  | 261.47 ms  | 19.91 MiB/s  | 22.92 MiB/s |
| **Interlaced 8bpp** | zune-png     | 138.86 ms  | 268.57 ms  | 72.64 MiB/s  | 37.56 MiB/s |
|                     | image-rs/png | 240.57 ms  | 360.80 ms  | 41.93 MiB/s  | 27.96 MiB/s |
|                     | spng         | 379.59 ms  | 376.09 ms  | 26.57 MiB/s  | 26.82 MiB/s |

---

## 5. QOI 
| Benchmark             | Library        | macOS Time | Linux Time | macOS Thrpt  | Linux Thrpt  |
|:----------------------|:---------------|:-----------|:-----------|:-------------|:-------------|
| **QOI Simple decode** | rapid-qoi      | 3.87 ms    | 7.00 ms    | 375.24 MiB/s | 207.40 MiB/s |
|                       | zune-qoi       | 6.98 ms    | 16.36 ms   | 207.76 MiB/s | 88.70 MiB/s  |

## 6. HEIF decode
| Benchmark       | Library        | macOS Time | Linux Time | macOS Thrpt | Linux Thrpt |
|:----------------|:---------------|:-----------|:-----------|:------------|:------------|
| **HEIF decode** | zune-heif (hw) | 33.47 ms   | —          | —           | —           |
|                 | zune-heif (sw) | 98.97 ms   | 255.47 ms  | —           | —           |
|                 | heic           | 119.60 ms  | 387.77 ms  | —           | —           |
## 6. Image Processing Operations
| Benchmark                | Library          | macOS Time | Linux Time | macOS Thrpt  | Linux Thrpt |
|:-------------------------|:-----------------|:-----------|:-----------|:-------------|:------------|
| **Affine transform 45°** | libvips / vips   | 48.27 ms   | 434.57 ms  | 25.17 MiB/s  | 2.80 MiB/s  |
|                          | zune-image       | 60.49 ms   | 368.68 ms  | 20.09 MiB/s  | 3.30 MiB/s  |
| **Sobel**                | libvips / vips   | 29.61 ms   | 144.52 ms  | 41.04 MiB/s  | 8.41 MiB/s  |
|                          | zune-image       | 16.89 ms   | 292.67 ms  | 71.96 MiB/s  | 4.15 MiB/s  |
| **Gamma**                | libvips / vips   | 9.62 ms    | 79.85 ms   | 126.36 MiB/s | 15.22 MiB/s |
|                          | zune-image       | 8.11 ms    | 86.24 ms   | 149.90 MiB/s | 14.09 MiB/s |
| **Gaussian blur**        | libvips / vips   | 27.33 ms   | 109.92 ms  | 44.46 MiB/s  | 11.06 MiB/s |
|                          | zune-image       | 41.172 ms  | 304.61 ms  | 29.515 MiB/s | 3.98 MiB/s  |
|                          | image-rs         | 221.33 ms  | 816.86 ms  | 5.49 MiB/s   | 1.49 MiB/s  |
| **Premultiply**          | libvips / vips   | 18.10 ms   | 288.84 ms  | 67.14 MiB/s  | 4.21 MiB/s  |
|                          | zune-image       | 3.42 ms    | 36.77 ms   | 354.93 MiB/s | 33.05 MiB/s |
| **Rotate 90**            | libvips / vips   | 16.43 ms   | 78.99 ms   | 73.95 MiB/s  | 15.38 MiB/s |
|                          | zune-image       | 20.79 ms   | 163.34 ms  | 58.46 MiB/s  | 7.44 MiB/s  |
|                          | image-rs         | 145.56 ms  | 183.57 ms  | 8.35 MiB/s   | 6.62 MiB/s  |
| **Rotate 180**           | libvips / vips   | 8.1493 ms  | 71.33 ms   | 149.12 MiB/s | 17.04 MiB/s |
|                          | zune-image       | 5.0725 ms  | 39.83 ms   | 239.57 MiB/s | 30.51 MiB/s |
| **Invert**               | libvips / vips   | 7.4999 ms  | 63.12 ms   | 162.03 MiB/s | 19.25 MiB/s |
|                          | zune-image       | 4.9397 ms  | 35.82 ms   | 246.00 MiB/s | 33.93 MiB/s |
| **Flip horizontal**      | libvips / vips   | 8.1941 ms  | 67.74 ms   | 148.30 MiB/s | 17.94 MiB/s |
|                          | zune-image       | 4.9439 ms  | 41.06 ms   | 245.80 MiB/s | 29.60 MiB/s |
| **Flip vertical**        | libvips / vips   | 5.1365 ms  | 56.87 ms   | 236.58 MiB/s | 21.37 MiB/s |
|                          | zune-image       | 4.9376 ms  | 38.87 ms   | 246.11 MiB/s | 31.27 MiB/s |
| **Resize linear**        | libvips / vips   | 9.4833 ms  | 41.81 ms   | 128.14 MiB/s | 29.07 MiB/s |
|                          | fir              | 26.091 ms  | 66.68 ms   | 46.575 MiB/s | 18.22 MiB/s |
|                          | zune-image       | 35.818 ms  | 233.02 ms  | 33.927 MiB/s | 5.21 MiB/s  |
|                          | image-rs         | 467.35 ms  | 1498.60 ms | 2.6001 MiB/s | 0.83 MiB/s  |
| **Resize lanczos**       | libvips / vips   | 13.65 ms   | 50.11 ms   | 89.00 MiB/s  | 24.25 MiB/s |
|                          | fir              | 28.27 ms   | 77.18 ms   | 42.98 MiB/s  | 15.74 MiB/s |
|                          | zune-image       | 43.83 ms   | 275.84 ms  | 27.72 MiB/s  | 4.41 MiB/s  |
|                          | image-rs         | 744.61 ms  | 2142.50 ms | 1.63 MiB/s   | 0.58 MiB/s  |
| **Resize mitchell**      | libvips / vips   | 11.71 ms   | 47.65 ms   | 103.78 MiB/s | 25.50 MiB/s |
|                          | fir              | 28.74 ms   | 70.78 ms   | 42.29 MiB/s  | 17.17 MiB/s |
|                          | zune-image       | 46.59 ms   | 293.26 ms  | 26.08 MiB/s  | 4.14 MiB/s  |
|                          | image-rs         | 729.33 ms  | 2144.80 ms | 1.67 MiB/s   | 0.58 MiB/s  | 
