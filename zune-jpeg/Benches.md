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

The library may be fast, okay it is fast.

But real world usage may vary.

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
| multithreaded                | ✅                     | ❌             | ✅         |
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

### 1 * 1 (No upsampling) Baseline RGB Decoding

| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 62.246 ms  |
| [libjpeg-turbo] | 98.343 ms  |
| [jpeg-decoder]  | 117.350 ms |

63% faster than libjpeg-turbo.

105% faster than image-rs/jpeg-decoder.

### Grayscale

### 1*1 Baseline grayscale decoding.

| Decoder       | Speed     |
|---------------|-----------|
| **zune-jpeg** | 45.598 ms |
| libjpeg-turbo | 46.648 ms |

Image-rs/jpeg-decoder does not support YCbCr->Grayscale decoding, hence it wasn't included in the benchmark

### Horizontal Sub-sampling

| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 50.246 ms  |
| [libjpeg-turbo] | 85.343 ms  |
| [jpeg-decoder]  | 118.350 ms |

70% faster than libjpeg-turbo.

136% faster than image-rs.

Image-rs remained almost the same compared to RGB no upsampling here.

### Vertical Sub-sampling

| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 50.175 ms  |
| [libjpeg-turbo] | 130.343 ms |
| [jpeg-decoder]  | 115.350 ms |

160% faster than libjpeg-turbo.

134% faster than image-rs.


### Horizontal Vertical Sub-sampling

This is probably the most common for low to medium quality images out there


| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 52.175 ms  |
| [libjpeg-turbo] | 78.343 ms  |
| [jpeg-decoder]  | 118.350 ms |

50% faster than libjpeg-turbo.

126% faster than image-rs.

# Apple M1

- Apple M1 2020
- 8Gb unified memory


### No upsampling  RGB Decoding

| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 44.246 ms  |
| [libjpeg-turbo] | 139.343 ms |
| [jpeg-decoder]  | 74.350 ms  |

### Horizontal Sub Sampling  RGB Decoding

| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 35.246 ms  |
| [libjpeg-turbo] | 121.343 ms |
| [jpeg-decoder]  | 76.350 ms  |

### Vertical Sub Sampling
| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 35.286 ms  |
| [libjpeg-turbo] | 161.343 ms |
| [jpeg-decoder]  | 73.350 ms  |

### HV Sub Sampling
| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 32.286 ms  |
| [libjpeg-turbo] | 141.343 ms |
| [jpeg-decoder]  | 82.350 ms  |

## Progressive decoding

Still incredibly spicy.

So here are the benchmarks.

### Apple M1

#### No upsampling  RGB Decoding

| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 141.246 ms |
| [libjpeg-turbo] | 246.343 ms |
| [jpeg-decoder]  | 257.350 ms |

#### Horizontal Sub Sampling  RGB Decoding

| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 115.246 ms |
| [libjpeg-turbo] | 198.343 ms |
| [jpeg-decoder]  | 211.350 ms |

#### Vertical Sub Sampling
| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 116.286 ms |
| [libjpeg-turbo] | 257.343 ms |
| [jpeg-decoder]  | 225.350 ms |

#### HV Sub Sampling

| Decoder         | Speed      |
|-----------------|------------|
| **zune-jpeg**   | 124.286 ms |
| [libjpeg-turbo] | 249.343 ms |
| [jpeg-decoder]  | 205.350 ms |




[libjpeg-turbo]:https://github.com/libjpeg-turbo/libjpeg-turbo
[jpeg-decoder]:https://github.com/image-rs/jpeg-decoder

[rayon]:https://github.com/rayon-rs/rayon