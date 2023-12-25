# zune-image

This workspace features a set of small,independent and performant image codecs that can be used
for decoding maniuplating and sometimes encoding images in a variety of formats.

The set of codecs aim to have the following features in order of priority

- Performance: Performance should be on par with or better than reference libraries. For example,
  `zune-jpeg` should easily replace `libjpeg-turbo` without any noticeable speed loss.
- Safety: Minimal `unsafe` code, with the sole exception of SIMD intrinsics which currently require `unsafe`.
- Robustness: All decoders should be fuzz tested and found bugs fixed promptly.
- Ease of use: Consistent API across decoders and encoders.
- Fast compile times: No dependencies on huge crates. Minimal and relatively well commented code.

## Features

- Single interface:
    - One image struct holds all the types, including 8-bit,16-bit animated, non-animated images, this
      makes your API easier.


- Fast :
    - Fast image decoders and encoders: `zune-*` image decoders are some of the fastest,
      see [benchmarks](https://etemesi254.github.io/posts/Zune-Benchmarks/)
    - Image filters are also optimized for speed
    - Benchmarks are prevalent to catch regressions, with crates like [zune-imageprocs](/crates/zune-imageprocs)
      containing micro-benchmarks for filters
    - Highly optimized simd routines, for things that need the extra juice,
      e.g [IDCT](https://github.com/etemesi254/zune-image/blob/2c4cb4e407a3c0a0aa50201ae1ba2c722e13cd8a/crates/zune-jpeg/src/idct/avx2.rs#L70)
      in [zune-jpeg](crates/zune-jpeg),
      [filter algorithms](https://github.com/etemesi254/zune-image/blob/2c4cb4e407a3c0a0aa50201ae1ba2c722e13cd8a/crates/zune-png/src/filters/sse4.rs#L175)
      in
      [zune-png](/crates/zune-png) and
      [alpha pre-multiplication](https://github.com/etemesi254/zune-image/blob/2c4cb4e407a3c0a0aa50201ae1ba2c722e13cd8a/crates/zune-imageprocs/src/premul_alpha/std_simd.rs#L11)
      (using portable simd)
      in [zune-imageprocs](crates/zune-imageprocs)
    - We utilize multiple threads to speed up compute heavy operations


- Extensive
    - Support for `u8`, `u16` and `f32` images. This means we support HDR image processing. This isn't limited to image
      decoders and encoders but image filters support it to.
    - Image decoders preserve image depth up until you want to change it, this means hdr is handled as hdr, 16-bit png
      is handled as 16 bit png.
    - Multiple image filters, we have common image manipulation filters like exposure, contrast, HSL, blurring, computer
      vision filters like sobel, mean filter, bilateral filters
    - Multiple color conversion routines which preserve bit depth, one can go from CMYK to HLS as easy
      as `image.convert_color`
    - A lot of testing, lossless decoders have bit-identical tests with other decoders, lossy decoders have their own
      type,
      see more on [Adding a Test](/docs/AddingATest.md) on how we do that


- Easy to use api
    - Image decoders implement `decode_headers` which allows one to retrieve image information without decoding the
      image
    - All image decoders implement `new` and `new_with_options`, with the former using default options and the latter
      using custom options
      allowing you to customize decoding
    - Image decoders implement common functions like `depth` for image depth, `colorspace` for image
      colorspace, `dimensions` for image dimensions
    - All image operations implement `OperationsTrait`, decoders `DecoderTrait` and encoders `EncoderTrait`


- Safe
    - We (99.9%) won't segfault on you, unless you do something silly.
    - Decoders are fuzz tested in CI when a feature is added and also fuzz tested every day to catch bugs.
    - Safety is kept to almost zero in most crates, with some having `#![forbid(unsafe)]` most unsafe comes from `SIMD`
      routines which will reduce when [portable-simd](https://github.com/rust-lang/portable-simd) becomes mainstream
    - Image crashes are treated with the seriousness they deserve, i.e we fix as quickly as possible and
      acknowledge,whether it's a less common decoder or a useful routine.


- A command line application.


- Bindings to other languages:
    - Python via [zune-python](/crates/zune-python)
    - C-bindings via [zune-capi](/crates/zune-capi)
    - JS/TS via [zune-wasm](/crates/zune-wasm)

- (Limited) support for animated images

## Formats

| Image Format | Decoder       | Encoder        | `no_std` Support |
|--------------|---------------|----------------|------------------|
| jpeg         | zune-jpeg     | [jpeg-encoder] | Yes              |
| png          | zune-png      | -              | Yes              |
| ppm          | zune-ppm      | zune-ppm       | Yes              |
| qoi          | zune-qoi      | zune-qoi       | Yes              |
| farbfeld     | zune-farbfeld | zune-farbfeld  | Yes              |
| psd          | zune-psd      | -              | Yes              |
| jpeg-xl      | [jxl-oxide]   | zune-jpegxl    | Yes [^1]         |
| hdr          | zune-hdr      | zune-hdr       | No [^2]          |

- [^1] You lose threading capabilities.
- [^2] Lack of existence of `floor` and `exp` in the `core` library.

## Safety

This workspace **allows only 1 type of unsafe:** platform specific intrinsics (e.g. SIMD), and only where speed really
matters.

All other types are **explicitly forbidden.**

## Repository structure

- `crates` Contain main image code, each crate is prefixed with `zune-`.
  The crates are divided into image formats, like `zune-png` deals with `png` decoding
- `zune-imageprocs` deals with image processing routines, etc etc
- `tests`: Image testing routines, they mainly read from `test-images`
- `benchmarks`: Benchmarking routines, they test the library routines with other popular image libraries.
- `fuzz-corpus` : Some interesting image files used for fuzzing.
- `test-images`: Images for testing various aspects of the decoder
- `docs`: Documentation on various parts of the library

## Why yet another image library

Rust already has a good image library i.e https://github.com/image-rs/image

But I'll let the overall speed of operations (decoding, applying image operations like blurring) speak for itself when
compared to other implementations.

## Benchmarks.

Library benchmarks are available [online] and also reproducible offline

To reproduce benchmarks you can run the following commands

Tested, on Linux, but should work for most operating systems

```shell
git clone https://github.com/etemesi254/zune-image
cd ./zune-image
cargo bench --workspace
```

This will create a criterion directory in target which will contain benchmark
results of most image decoding operations.


[online]:https://etemesi254.github.io/posts/Zune-Benchmarks/

## Fuzzing

Most decoders are tested in CI to ensure new changes do not introduce regressions.

Critical decoders are fuzz tested in CI once every day to catch any potential issue/bug.


[jpeg-encoder]: https://github.com/vstroebel/jpeg-encoder

[jxl-oxide]: https://github.com/tirr-c/jxl-oxide