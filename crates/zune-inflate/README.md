# zune-inflate

This crate features an optimized inflate algorithm supporting
whole buffer decompression.

Supported formats are

- raw deflate
- zlib (deflate with a zlib wrapper on)

The implementation is heavily based on Eric Biggers [libdeflate] and hence
has similar characteristics.

Specifically, we do not support streaming decompression but prefer whole buffer decompression.

## Installation

To use in your crate, simply add the following in your
Cargo.toml

```toml
[dependencies]
#other amazing crates from other amazing people
zune-inflate = "0.2.0"
```

## Usage.

The library exposes a simple API for decompressing
data, and depending on what type of data you have, you typically choose
one of the `decode[_suffix]` function to decode your data

The decompressor expects the whole buffer handed upfront

### Decoding raw deflate

To decode raw deflate data, the following code should get you
started.

```rust
use zune_inflate::DeflateDecoder;
let totally_valid_data = [0; 23];
let mut decoder = DeflateDecoder::new( & totally_valid_data);
// panic on errors, because that's the cool way to go
let decompressed_data = decoder.decode_deflate().unwrap();
```

### Decoding zlib

To decode deflate data wrapped in zlib, the following code should get you
started.

```rust
use zune_inflate::DeflateDecoder;
let totally_valid_data = [0; 23];
let mut decoder = DeflateDecoder::new( & totally_valid_data);
// panic on errors, because that's the cool way to go
let decompressed_data = decoder.decode_zlib().unwrap();
```

### Advanced usage

There are advanced options specified by `DeflateOptions` which can change
decompression settings.

## Comparisions.

I'll compare this with `flate2` with `miniz-oxide` backend.

| feature                 | `zune-inflate` | `flate2` |
|-------------------------|----------------|----------|
| zlib decompression      | yes            | yes      |
| delfate decompression   | yes            | yes      |
| gzip                    | soon           | yes      |
| compression             | soon           | yes      |
| streaming decompression | no             | yes      |
| **unsafe**              | no             | yes      |

As you can see, there are a lot of features we currently lack when compared to
flate2/miniz-oxide.

There's actually nothing riding in for us, except...it's wickedly fast...

### Benchmarks

Again, I'm gonna compare zune-inflate and miniz-oxide.

The test bench is a 42mb enwiki file compressed to 13.3 mb of zlib
infused madness, we test decompression on the machine

| File        | Metric     | Zune-Inflate | flate2/miniz-oxide | libdeflate  |
|-------------|------------|--------------|--------------------|-------------|
| enwiki_part | Speed      | 86.295 ms    | 158.11 ms          | 63.745ms    |
|             | Throughput | 146.83 Mb/s  | 84.230 Mb/s        | 208.92 Mb/s |

- libdeflate: Provided by [libdeflater] which offers bindings to [libdeflate]

 <small> Damn impressive libdeflate, damn impressive</small>

## Fuzzing

The decoder is currently fuzzed for correctness by both `miniz-oxide` and `zlib-ng`, see the fuzz/src directory

[libdeflater]: https://github.com/adamkewley/libdeflater

[libdeflate]:https://github.com/ebiggers/libdeflate
