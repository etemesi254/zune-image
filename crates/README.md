# zune-image

This workspace features a set of small,independent and performant image codecs that can be used
for decoding and sometimes encoding images in a variety of formats.

The set of codecs aim to have the following features in order of priority

- Performance: Performance should be almost similar to reference libraries, this means that
  `zune-jpeg` should easily replace `libjpeg-turbo` without any noticeable speed loss.
- Safety: All decoders should be fuzz tested and such bugs fixed promptly.
- Ease of use: Consistent API across decoders and encoders.
  Anyone, even your grandma should be able to decode supported formats
- Fast compile times: No dependencies on huge crates, minimal (relatively well commented) code

## Safety

While it is quite possible to implement all decoders in 100% safe Rust, it is sometimes required
to dabble in the arts of `unsafe` Rust when speed matters.

But again we can abuse the notion of the search for the fastest code to write some crabby code and justify it with
benchmarks.

Which beats the purpose of using a memory safe language, but just as with life, compromises have to be made.

This workspace **allows only 1 type of unsafe**

- Platform specific intrinsics, where speed matters.

All other types are **explicitly forbidden**

## Why yet another image library

Rust already has a good image library i.e https://github.com/image-rs/image
and there is probably no reason to have this,

But I'll let the overall speed of operations(decoding, applying image operations like blurring) speak for itself when
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
