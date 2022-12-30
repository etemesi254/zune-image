# Benchmarks of popular jpeg libraries

Here I compare how long it takes popular JPEG decoders to decode the below 7680*4320 image
of [Cutefish OS](https://en.cutefishos.com/) default wallpaper.
![img](benches/images/speed_bench.jpg)

## About benchmarks

Benchmarks are weird, especially IO & multi-threaded programs. This library uses both of the above hence performance may
vary.

For best results shut down your machine, go take coffee, think about life and how it came to be and why people should
save the environment.

Then power up your machine, if it's a laptop connect it to a power supply and if there is a setting for performance
mode, tweak it.

Then run.

## Benchmarks vs real world usage

Real world usage may vary.

Notice that I'm using a large image but probably most decoding will be small to medium images.

To make the library thread safe, we do about 1.5-1.7x more allocations than libjpeg-turbo. Although, do note that the
allocations do not occur at ago, we allocate when needed and deallocate when not needed.

Do note if memory bandwidth is a limitation. This is not for you.

## Reproducibility

The benchmarks are carried out on my local machine with an AMD Ryzen 5 4500u

The benchmarks are reproducible.

To reproduce them

1. Clone this repository
2. Install rust(if you don't have it yet)
3. `cd` into the directory.
4. Run `cargo bench`

## Performance features of the three libraries

| feature                      | image-rs/jpeg-decoder | libjpeg-turbo | zune-jpeg |
|------------------------------|-----------------------|---------------|-----------|
| multithreaded                | ✅                     | ❌             | ❌         |
| platform specific intrinsics | ✅                     | ✅             | ✅         |

- Image-rs/jpeg-decoder uses [rayon] under the hood but it's under a feature
  flag.

- libjpeg-turbo uses hand-written asm for platform specific intrinsics, ported to
  the most common architectures out there but falls back to scalar
  code if it can't run in a platform.

# Finally benchmarks

## x86_64

#### Machine Specs

- Model name:          AMD Ryzen 5 4500U with Radeon Graphics
- CPU family:          23
- Model:               96


- Thread(s) per core:  1
- Core(s) per socket:  6


- L1d:                   192 KiB (6 instances)
- L1i:                   192 KiB (6 instances)
- L2:                    3 MiB (6 instances)
- L3:                    8 MiB (2 instances)

###     

| Benchmark name                      | zune-jpeg | mozjpeg   | image-rs/jpeg-decoder |
|-------------------------------------|-----------|-----------|-----------------------|
| No sampling/Baseline JPEG Decoding  | 101.15 ms | 100.56 ms | 177.20 ms             |
| Horizontal Sub sampling 2V1         | 93.501 ms | 92.514 ms | 123.50 ms             |
| Vertical sub sampling 2V1           | 98.996 ms | 138.03 ms | 120.05 ms             |
| HV sampling (2V2)                   | 96.982 ms | 89.644ms  | 107.61 ms             |
| Grayscale                           | 54.420 ms | 43.094 ms | -                     |
| Progressive 1V1                     | 312.68 ms | 302.02 ms | 468.63 ms             |
| Progressive Horizontal sub-sampling | 235.19 ms | 214.50 ms | 352.94 ms             |
| Progressive Vertical Sub Sampling   | 238.71 ms | 269.79 ms | 333.90 ms             |
| Progressive HV sampling             | 224.46 ms | 265.36 ms | 350.00 ms             |
| APPROX TOTAL                        | 1451 ms   | 1512 ms   | 2030 ms*              |

* Without grayscale sum

| Benchmark          | zune-jpeg |
|--------------------|-----------|
| allowed intrinsics | 107.03 ms |
| no-intrinsics      | 132.43 ms |

[libjpeg-turbo]:https://github.com/libjpeg-turbo/libjpeg-turbo

[jpeg-decoder]:https://github.com/image-rs/jpeg-decoder

[rayon]:https://github.com/rayon-rs/rayon