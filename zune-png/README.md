## Zune-png

An incredibly spicy png decoder.

This crate features a png decoder

## Limitations

- This decoder (currently) expands images with less than 8 bpp to be 8 bits(one byte)
  automatically.
  This may or may not be desired depending on your use cases

## Features

- Fast deflate decoder
- Vectorized code paths
- Minimal number of huge allocations(mainly 2 for most images, 3 for interlaced)
- Zero unsafe outside of platform specific intriniscs

## Usages

First, include this in your Cargo.toml

```toml
[dependencies]
zune-png = "0.2.0"
```

Then you can access the decoder in your library/binary.

```rust
use zune_png::Png;
```
