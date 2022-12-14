# Zune-JPEG

A fast and safe jpeg decoder.

This repo contains a pure Rust jpeg decoder

## Usage

The library provides a simple-to-use API for jpeg decoding
and an ability to add options to influence decoding

### Example

```rust
// Import the library
use zune_jpeg::JpegDecoder;
use std::fs::read;
// load some jpeg data
let data = read("cat.jpg").unwrap();

// create a decoder
let mut decoder = JpegDecoder::new( & data);
// decode the file
let pixels = decoder.decode().unwrap();
```

The decoder supports more manipulations via `ZuneJpegOptions`,
see additional documentation in the library.

## Goals

The implementation aims to have the following goals achieved,
in order of importance

1. Safety - Do not segfault on errors or invalid input, panics are okay, but
   should be fixed when reported. `unsafe` should be used sparingly
2. Speed - Get the data as quickly as possible, which means
    1. Platform intrinsics code where justifiable
    2. Carefully written platform independent code that allows the
       compiler to vectorize it.
    3. Regression tests.
    4. Watch the memory usage of the program
3. Usability - Provide utility functions like different color conversions functions.

## Non-Goals

- Bit identical results with libjpeg/libjpeg-turbo will never be an aim of this library.
  Jpeg is a lossy format with very few parts specified by the standard(
  i.e it doesn't give a reference upsampling and color conversion algorithm)
- Error recovery - This is not a recovery library, most errors are propagated up to the caller

## Features

- [x] A Pretty fast 8*8 integer IDCT.
- [x] Fast Huffman Decoding
- [x] Fast color convert functions.
- [x] Support for extended colorspaces like GrayScale and RGBA
- [X] Single-threaded decoding.

# Crate Features

| feature | on  | Capabilities                                                                                |
|---------|-----|---------------------------------------------------------------------------------------------|
| `x86`   | yes | Enables `x86` specific instructions, specifically `avx` and `sse` for accelerated decoding. |

Note that the `x86` features are automatically disabled on platforms that aren't x86 during compile
time hence there is no need to disable them explicitly if you are targeting such a platform.

## Debug vs release

The decoder heavily relies on platform specific intrinsics, namely AVX2 and SSE to gain speed-ups in decoding,
but in debug build rust generally [doesn't like platform specific intrinsics](https://godbolt.org/z/vPq57z13b) (try
passing `-O` parameter to see optimized build) hence obviously speeds tank so bad during debug builds, and there is
probably nothing
we can do about that.

## Benchmarks

The library tries to be at fast as [libjpeg-turbo] while being as safe as possible.
Platform specific intrinsics help get speed up intensive operations ensuring we can almost
match [libjpeg-turbo] speeds but speeds are always +- 10 ms of this library.

For more up-to-date benchmarks, see [Benches.md](/zune-jpeg/Benches.md).

# TODO

- [ ] Add support for Adobe APP14 images.
- [ ] Support more colorspace options. It would not be too bad if we support all color options libjpeg/mozjpeg supports.

[libjpeg-turbo]:https://github.com/libjpeg-turbo/libjpeg-turbo/

[image-rs/jpeg-decoder]:https://github.com/image-rs/jpeg-decoder/tree/master/src