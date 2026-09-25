# Criterion benchmark results

## Benchmarks

| Benchmark         | Library                      | Darwin Time | Darwin Throughput | Darwin Comparison |
|:------------------|:-----------------------------|:------------|:------------------|:------------------|
| **HEIF Decoding** | heic                         | 126.8 ms    | —                 | 3.97× slower      |
|                   | zune-heif (hardware decoder) | 31.94 ms    | —                 | **Winner**        |
|                   | zune-heif (software decoder) | 105.1 ms    | —                 | 3.29× slower      |

## Hdr

| Benchmark                       | Library           | Darwin Time | Darwin Throughput | Darwin Comparison |
|:--------------------------------|:------------------|:------------|:------------------|:------------------|
| **io**                          | hdr: file io      | 5.12 ms     | —                 | 2.72× slower      |
|                                 | hdr: in-memory io | 1.88 ms     | —                 | **Winner**        |
| **Simple decode(memorial-hdr)** | image-rs/hdr      | 880.8 µs    | 1.42 GiB/s        | **Winner**        |
|                                 | zune-image/hdr    | 5.07 ms     | 252.8 MiB/s       | 5.75× slower      |

## Imageprocs

| Benchmark                                  | Library                                 | Darwin Time | Darwin Throughput | Darwin Comparison |
|:-------------------------------------------|:----------------------------------------|:------------|:------------------|:------------------|
| **affine-transform (45 degrees rotation)** | libvips                                 | 49.23 ms    | 24.69 MiB/s       | **Winner**        |
|                                            | zune-image                              | 59.52 ms    | 20.41 MiB/s       | 1.21× slower      |
| **flip-horizontal**                        | libvips                                 | 8.26 ms     | 147.0 MiB/s       | 1.68× slower      |
|                                            | zune-image                              | 4.92 ms     | 246.9 MiB/s       | **Winner**        |
| **flip-vertical**                          | libvips                                 | 5.29 ms     | 229.6 MiB/s       | 1.061× slower     |
|                                            | zune-image                              | 4.99 ms     | 243.5 MiB/s       | **Winner**        |
| **gamma**                                  | libvips                                 | 10.00 ms    | 121.5 MiB/s       | 1.25× slower      |
|                                            | zune-image                              | 7.97 ms     | 152.4 MiB/s       | **Winner**        |
| **gaussian blur**                          | image-rs                                | 218.8 ms    | 5.55 MiB/s        | 7.96× slower      |
|                                            | vips                                    | 27.48 ms    | 44.21 MiB/s       | **Winner**        |
|                                            | zune-image                              | 27.57 ms    | 44.07 MiB/s       | 1.003× slower     |
| **gaussian blur - fast approximation**     | libblur (interleaved mode (3 channels)) | 20.38 ms    | —                 | **Winner**        |
|                                            | libblur (planar mode 3 passes)          | 36.87 ms    | —                 | 1.81× slower      |
|                                            | zune-image                              | 24.10 ms    | —                 | 1.18× slower      |
| **invert**                                 | libvips                                 | 6.78 ms     | 179.2 MiB/s       | 1.39× slower      |
|                                            | zune-image                              | 4.87 ms     | 249.7 MiB/s       | **Winner**        |
| **median blur**                            | libblur                                 | 786.6 ms    | —                 | **Winner**        |
|                                            | zune-image                              | 879.1 ms    | —                 | 1.12× slower      |
| **premultiply**                            | libvips                                 | 18.49 ms    | 65.71 MiB/s       | 5.32× slower      |
|                                            | zune-image                              | 3.48 ms     | 349.7 MiB/s       | **Winner**        |
| **resize - lanczos-kernel**                | fir                                     | 27.89 ms    | 43.57 MiB/s       | 1.88× slower      |
|                                            | image-rs                                | 724.4 ms    | 1.68 MiB/s        | 48.84× slower     |
|                                            | vips                                    | 14.83 ms    | 81.93 MiB/s       | **Winner**        |
|                                            | zune-image                              | 43.00 ms    | 28.26 MiB/s       | 2.90× slower      |
| **resize - mitchell**                      | fir                                     | 29.41 ms    | 41.32 MiB/s       | 2.45× slower      |
|                                            | image-rs                                | 738.5 ms    | 1.65 MiB/s        | 61.44× slower     |
|                                            | vips                                    | 12.02 ms    | 101.1 MiB/s       | **Winner**        |
|                                            | zune-image                              | 46.35 ms    | 26.22 MiB/s       | 3.86× slower      |
| **resize-linear-kernel**                   | fir                                     | 28.79 ms    | 42.20 MiB/s       | 2.93× slower      |
|                                            | image-rs                                | 486.1 ms    | 2.50 MiB/s        | 49.42× slower     |
|                                            | vips                                    | 9.84 ms     | 123.5 MiB/s       | **Winner**        |
|                                            | zune-image                              | 41.61 ms    | 29.20 MiB/s       | 4.23× slower      |
| **rotate 180**                             | libvips                                 | 8.30 ms     | 146.4 MiB/s       | 1.64× slower      |
|                                            | zune-image                              | 5.06 ms     | 240.0 MiB/s       | **Winner**        |
| **rotate 90**                              | image-rs                                | 146.7 ms    | 8.28 MiB/s        | 8.04× slower      |
|                                            | vips                                    | 18.24 ms    | 66.61 MiB/s       | **Winner**        |
|                                            | zune-image                              | 21.85 ms    | 55.62 MiB/s       | 1.20× slower      |
| **sobel**                                  | libvips                                 | 29.85 ms    | 40.72 MiB/s       | 1.82× slower      |
|                                            | zune-image                              | 16.40 ms    | 74.09 MiB/s       | **Winner**        |
| **transpose**                              | zune-image                              | 7.81 ms     | —                 | **Winner**        |

## Inflate

| Benchmark                                | Library       | Darwin Time | Darwin Throughput | Darwin Comparison |
|:-----------------------------------------|:--------------|:------------|:------------------|:------------------|
| **enwiki zlib decoding**                 | flate/zlib-ng | 123.1 ms    | 108.2 MiB/s       | 3.24× slower      |
|                                          | libdeflate    | 37.99 ms    | 350.6 MiB/s       | **Winner**        |
|                                          | zune-inflate  | 65.40 ms    | 203.6 MiB/s       | 1.72× slower      |
| **gzip decoding, image-rs rustdoc json** | flate/zlib-ng | 3.95 ms     | 101.5 MiB/s       | 2.57× slower      |
|                                          | libdeflate    | 1.54 ms     | 261.0 MiB/s       | **Winner**        |
|                                          | zune-inflate  | 3.85 ms     | 104.1 MiB/s       | 2.51× slower      |
| **gzip decoding, tokio-rs source code**  | flate/zlib-ng | 37.48 ms    | 339.2 MiB/s       | 1.71× slower      |
|                                          | libdeflate    | 21.88 ms    | 581.2 MiB/s       | **Winner**        |
|                                          | zune-inflate  | 36.33 ms    | 349.9 MiB/s       | 1.66× slower      |
| **zlib decoding-png zlib**               | flate/zlib-ng | 101.5 ms    | 116.3 MiB/s       | 2.83× slower      |
|                                          | libdeflate    | 35.83 ms    | 329.3 MiB/s       | **Winner**        |
|                                          | zune-inflate  | 54.85 ms    | 215.1 MiB/s       | 1.53× slower      |

## Jpeg

| Benchmark                               | Library                      | Darwin Time | Darwin Throughput | Darwin Comparison |
|:----------------------------------------|:-----------------------------|:------------|:------------------|:------------------|
| **DRI / Restart markers**               | incremental resume           | 6.36 ms     | 21.43 MiB/s       | 1.77× slower      |
|                                         | one-shot                     | 3.60 ms     | 37.86 MiB/s       | **Winner**        |
| **Grayscale decoding**                  | mozjpeg                      | 28.64 ms    | 42.43 MiB/s       | **Winner**        |
|                                         | zune-jpeg                    | 37.96 ms    | 32.01 MiB/s       | 1.33× slower      |
| **Horizontal Sub Sampling**             | mozjpeg                      | 41.06 ms    | 26.09 MiB/s       | **Winner**        |
|                                         | zune-jpeg                    | 50.68 ms    | 21.14 MiB/s       | 1.23× slower      |
| **HV sampling**                         | mozjpeg                      | 36.28 ms    | 26.93 MiB/s       | **Winner**        |
|                                         | zune-jpeg                    | 48.31 ms    | 20.23 MiB/s       | 1.33× slower      |
| **Incremental mode checkpoints**        | first retry default          | 84.38 ms    | 11.58 MiB/s       | 1.77× slower      |
|                                         | first retry incremental-mode | 47.58 ms    | 20.54 MiB/s       | **Winner**        |
|                                         | one-shot default             | 48.49 ms    | 20.15 MiB/s       | 1.019× slower     |
|                                         | one-shot incremental-mode    | 49.50 ms    | 19.74 MiB/s       | 1.040× slower     |
| **No sampling Baseline decode**         | mozjpeg                      | 50.00 ms    | 24.30 MiB/s       | **Winner**        |
|                                         | zune-jpeg                    | 54.83 ms    | 22.16 MiB/s       | 1.097× slower     |
| **No sampling Progressive decoding**    | mozjpeg                      | 121.2 ms    | 10.92 MiB/s       | 1.099× slower     |
|                                         | zune-jpeg                    | 110.3 ms    | 11.99 MiB/s       | **Winner**        |
| **Progressive Horizontal Sub Sampling** | mozjpeg                      | 97.23 ms    | 11.89 MiB/s       | 1.009× slower     |
|                                         | zune-jpeg                    | 96.36 ms    | 12.00 MiB/s       | **Winner**        |
| **Progressive HV sampling**             | mozjpeg                      | 95.18 ms    | 12.04 MiB/s       | **Winner**        |
|                                         | zune-jpeg                    | 98.16 ms    | 11.67 MiB/s       | 1.031× slower     |
| **Progressive Vertical sub sampling**   | mozjpeg                      | 95.10 ms    | 12.05 MiB/s       | **Winner**        |
|                                         | zune-jpeg                    | 98.20 ms    | 11.67 MiB/s       | 1.033× slower     |
| **Vertical sub sampling**               | mozjpeg                      | 40.84 ms    | 26.10 MiB/s       | **Winner**        |
|                                         | zune-jpeg                    | 54.55 ms    | 19.54 MiB/s       | 1.34× slower      |
| **zune-jpeg Intrinsics**                | intrinsics                   | 54.81 ms    | 22.17 MiB/s       | **Winner**        |
|                                         | no intrinsics                | 55.44 ms    | 21.92 MiB/s       | 1.011× slower     |

## Png

| Benchmark                        | Library      | Darwin Time | Darwin Throughput | Darwin Comparison |
|:---------------------------------|:-------------|:------------|:------------------|:------------------|
| **PNG decoding  16 bpp**         | image-rs/png | 163.7 ms    | 46.92 MiB/s       | **Winner**        |
|                                  | spng         | 541.2 ms    | 14.19 MiB/s       | 3.31× slower      |
|                                  | zune-png     | 186.3 ms    | 41.23 MiB/s       | 1.14× slower      |
| **PNG decoding baseline**        | image-rs/png | 149.3 ms    | 40.12 MiB/s       | 1.11× slower      |
|                                  | spng         | 297.4 ms    | 20.15 MiB/s       | 2.20× slower      |
|                                  | zune-png     | 135.0 ms    | 44.38 MiB/s       | **Winner**        |
| **PNG decoding interlaced 8bpp** | image-rs/png | 232.8 ms    | 43.33 MiB/s       | 1.67× slower      |
|                                  | spng         | 378.0 ms    | 26.69 MiB/s       | 2.71× slower      |
|                                  | zune-png     | 139.7 ms    | 72.22 MiB/s       | **Winner**        |
| **PNG decoding palette image**   | image-rs/png | 28.31 ms    | 184.1 MiB/s       | **Winner**        |
|                                  | spng         | 53.59 ms    | 97.27 MiB/s       | 1.89× slower      |
|                                  | zune-png     | 35.21 ms    | 148.0 MiB/s       | 1.24× slower      |

## Qoi

| Benchmark         | Library   | Darwin Time | Darwin Throughput | Darwin Comparison |
|:------------------|:----------|:------------|:------------------|:------------------|
| **Simple decode** | rapid-qoi | 3.81 ms     | 380.3 MiB/s       | **Winner**        |
|                   | zune-qoi  | 7.18 ms     | 202.1 MiB/s       | 1.88× slower      |

