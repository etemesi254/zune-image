# Zune-HEIC

A fast, correct and safe HEIF/HEIC decoder in pure Rust, with its own HEVC decoder.

It decodes HEIF still images coded with HEVC (H.265): the `.heic` / `.heif` files
taken by iPhones and many other cameras. There are no C dependencies.

## Usage

Add the crate to your `Cargo.toml`

```toml
[dependencies]
zune-heic = "0.5"
```

### Example

```Rust
use zune_heic::HeifDecoder;
use zune_heic::zune_core::bytestream::ZCursor;

fn main() -> Result<(), zune_heic::HeicErrors> {
    // load some heic data
    let data = std::fs::read("image.heic").unwrap();
    // create the decoder
    let mut decoder = HeifDecoder::new(ZCursor::new(&data));
    // decode to pixels, interleaved 8-bit RGB (or RGBA / Luma / LumaA)
    let pixels = decoder.decode()?;

    let width = decoder.width().unwrap();
    let height = decoder.height().unwrap();
    let colorspace = decoder.colorspace().unwrap();
    assert_eq!(pixels.len(), width * height * colorspace.num_components());
    Ok(())
}
```

The decoder can also read straight from a file (`BufReader<File>`), decode only the
headers to get the dimensions (`decode_headers`), decode into a buffer you already
have (`decode_into` + `output_buffer_size`) and return the Exif and ICC metadata
(`exif_data`, `icc_data`).

Decoding can be configured with `DecoderOptions`, e.g.

```Rust
use zune_heic::zune_core::options::DecoderOptions;

let options = DecoderOptions::default()
    // use the software decoder on macOS too
    .hvec_set_use_videotoolbox(false)
    // threads used to decode grid tiles in parallel
    .set_num_threads(8)
    // refuse images larger than this
    .set_max_width(16384)
    .set_max_height(16384);

let mut decoder = HeifDecoder::new_with_options(ZCursor::new(&data), options);
```

See the [crate documentation](https://docs.rs/zune-heic) for more examples.

## Goals

The implementation aims to have the following goals achieved,
in order of importance

1. Correctness - The output is bit-identical to [libheif]: the decoded RGB matches
   libheif byte for byte, and the decoded YCbCr of every coded picture matches
   [libde265]. Both are checked in CI on every change.
2. Safety - Do not segfault on errors or invalid input. The decoder is safe Rust;
   `unsafe` is only used for SIMD intrinsics and the Apple VideoToolbox bindings.
3. Speed - Decode as fast as possible, which means
    1. Platform intrinsics where justifiable
    2. Multi-threading where the format allows it
    3. Not copying the image around more than needed

## Features

- [x] Bit-identical output to libheif 1.17.
- [x] Fast SIMD inverse transforms (IDCT/IDST): SSE4.1 on x86_64 and NEON on aarch64,
  selected at runtime, with a portable fallback.
- [x] Multi-threaded decoding: grid images (like iPhone photos, made of 512×512 tiles)
  decode their tiles in parallel, single images decode CTU rows in parallel when the
  stream allows it (wavefront parallel processing), and deblocking and SAO are split
  over the CPUs too.
- [x] Hardware decoding on macOS through Apple VideoToolbox (on by default).
- [x] Grid images.
- [x] Clean aperture cropping (`clap`), rotation (`irot`) and mirroring (`imir`).
- [x] Alpha planes, including premultiplied alpha detection.
- [x] Greyscale (monochrome) images.
- [x] Colour description (`colr` nclx, or the HEVC VUI) for the YCbCr → RGB conversion.
- [x] Exif and ICC profile extraction.
- [x] Support for `no_std`.

## Crate Features

| feature      | on  | Capabilities                                                                                                                                                      |
|--------------|-----|-------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `std`        | yes | Multi-threaded decoding, runtime CPU feature detection for the SIMD transforms, reading from `std::io` types and the Apple VideoToolbox decoder.                  |
| `simd`       | no  | Use `core::simd` (portable SIMD) transforms, requires a nightly compiler.                                                                                        |
| `dump-tiles` | no  | `HeifDecoder::decode_tiles_yuv`: every coded picture's HEVC bitstream and decoded YCbCr planes, for testing the decoder against a reference decoder.              |

## Using in a `no_std` environment

The crate can be used in a `no_std` environment with the `alloc` crate, by disabling
the `std` feature. Decoding is then single-threaded, and the SIMD transforms are chosen
at compile time (`-C target-feature`).

One is required to link to a working allocator for whatever environment the decoder
will be running on.

## Debug vs release

The decoder relies on optimizations and platform intrinsics to be fast, and is many
times slower in debug builds. To get reasonable performance even when compiling your
program in debug mode, add this to your `Cargo.toml`:

```toml
# `zune-heic` package will be always built with optimizations
[profile.dev.package.zune-heic]
opt-level = 3
```

## Benchmarks

Decoding an iPhone photo (`fuzz-samples/IMG_4862.HEIC`, a 4284×5712 grid of 49 tiles)
to RGB on an Apple M3 Pro (18 GB RAM), measured with
`cargo bench -p zune-benches --bench decode_heic --features libheif`
([`benchmarks/benches/decode_heic.rs`](../../benchmarks/benches/decode_heic.rs); the `libheif`
feature needs the system libheif, e.g. `brew install libheif` or `apt install libheif-dev`):

| Decoder                                                    | Time        |
|------------------------------------------------------------|-------------|
| zune-heic, VideoToolbox (hardware)                         | 43.5 ms     |
| **zune-heic, software**                                    | **84.2 ms** |
| [heic](https://crates.io/crates/heic) crate (pure Rust)    | 114.7 ms    |
| [libheif] (through `libheif-rs`)                           | 202.7 ms    |

The software decoder is about 2.4× faster than libheif and 1.4× faster than the `heic`
crate; only the hardware decoder is faster.

On a 2-core x86_64 machine (Xeon @ 2.8 GHz, Linux), against libheif 1.17.6 with
libde265 1.0.15 (`heif_decode_image` with its default settings), best of 7 runs, on the
samples in `fuzz-samples/`:

| Image                     | Size      | libheif | zune-heic |
|---------------------------|-----------|---------|-----------|
| iPhone photo, 49-tile grid| 4284×5712 | 569 ms  | 384 ms    |
| grid, 48 tiles            | 4000×3000 | 368 ms  | 284 ms    |
| single image              | 2880×1800 | 243 ms  | 119 ms    |
| single image              | 1920×1920 | 115 ms  | 42 ms     |
| single image              | 800×800   | 86 ms   | 61 ms     |

## Limitations

- Only the primary image is decoded (no thumbnails, depth maps or image sequences).
- 8-bit, intra-coded HEVC only (what HEIF still images use), in 4:2:0 or monochrome;
  10-bit, 4:2:2 and 4:4:4 images are rejected.
- AVIF (AV1-coded HEIF) is not supported.
- Premultiplied alpha is returned as stored, like libheif does.
- With VideoToolbox, the YCbCr → RGB conversion is the one macOS uses, so the output is
  not bit-identical to libheif. Disable it with `hvec_set_use_videotoolbox(false)` if
  that matters.

## Patents

HEIC images are compressed with HEVC (H.265), and HEVC is covered by patents in many
countries. Licenses for these patents are offered by patent pools (such as Access
Advance and Via LA) and by individual patent holders.

This crate's license covers the source code only; it does not grant any patent rights.
Whether you need a patent license to use, ship or sell software that includes this
decoder depends on your product, your use of it and the countries involved. Obtaining
any such license is your responsibility. If in doubt, consult a lawyer; this notice is
not legal advice.

## License

Licensed under any of [MIT](LICENSE-MIT), [Apache-2.0](LICENSE-APACHE) or
[Zlib](LICENSE-ZLIB), at your option.

[libheif]: https://github.com/strukturag/libheif

[libde265]: https://github.com/strukturag/libde265
