# zune-image

This is pre alpha pre beta, think of it like a leaked code.

This contains a POC code of some image stuff, very much WIP and has too many things not working,
and some working.

It aims to have the following features

- Safety(unsafe is allowed, but used sparingly)
- Fast decode speeds
- Fast image operation speeds
- Use explicit SIMD where applicable(for cases where speed is important)
- Otherwise, trust the compiler.
- Small dependency footprint.

## Why yet another image library

Rust already has a good image library i.e https://github.com/image-rs/image
and there is probably no reason to have this,

But I'll let the overall speed of operations(decoding, applying image operations like blurring) speak for itself when
compared to other implementations.

## Library organization

- `zune-bin`: Provides a simple cmd application you can use for simple image manipulations

- `zune-image_format`: Provides an image decoder(and sometimes encoder) for that specific image format
  e.g `zune-jpeg` has a jpeg decoder, `zune-ppm` has a ppm decoder.
  Each decoder is independent of the rest of the decoders so one can pick whatever suits them.
- `zune-imageprocs`: Image processing routines, raw processing routines like `brighten` operations that work on native
  rust types
  (`u8` & `u16`) and are independent of image format.
- `zune-image`: A combined library relying on decoders and encoders and image processing routines that brings together
  the whole set together.
  allows one to slice , dice and splice images and create whatever suits you.
- `zune-core`: Core routines required by(and shared with) multiple image formats.

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
