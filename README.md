# zune-image

This workspace features a set of small,independent and performant image codecs that can be used
for decoding and sometimes encoding images in a variety of formats.

The set of codecs aim to have the following features in order of priority

- Performance: Performance should be on par with or better than reference libraries. For example,
  `zune-jpeg` should easily replace `libjpeg-turbo` without any noticeable speed loss.
- Safety: No `unsafe` code, with the sole exception of SIMD intrinsics which currently require `unsafe`.
- Robustness: All decoders should be fuzz tested and found bugs fixed promptly.
- Ease of use: Consistent API across decoders and encoders.
  Anyone, even your grandma should be able to decode supported formats
- Fast compile times: No dependencies on huge crates. Minimal and relatively well commented code.

## Formats

| Image Format | Decoder       | Encoder        |
|--------------|---------------|----------------|
| jpeg         | zune-jpeg     | [jpeg-encoder] |
| png          | zune-png      | -              |
| ppm          | zune-ppm      | zune-ppm       |
| qoi          | zune-qoi      | zune-qoi       |
| farbfeld     | zune-farbfeld | zune-farbfeld  |
| psd          | zune-psd      | -              |
| jpeg-xl      | -             | zune-jpegxl    |
| hdr          | zune-hdr      | zune-hdr       |

## Safety

This workspace **allows only 1 type of unsafe:** platform specific intrinsics (e.g. SIMD), and only where speed really
matters.

All other types are **explicitly forbidden.**

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
