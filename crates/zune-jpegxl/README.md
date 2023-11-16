## zune-jpegxl

A simple jpeg-xl encoder

This features a simple jpeg-xl lossless encoder with the following features

- Lossless encoding
- 8 bit and 16 bit support
- Grayscale and RGB{A} encoding
- Threading capabilities

## Usage

First add the latest into your cargo toml

By `cargo add`

```shell
cargo add zune-jpegxl
```

Or adding directly to your `Cargo.toml`

```toml
zune-jpegxl = "0.4"
```

Then use the `JxlSimpleEncoder` struct to encode an image

```rust
use zune_core::bit_depth::BitDepth;
use zune_core::options::EncoderOptions;
use zune_jpegxl::JxlSimpleEncoder;
// this example won't work
fn main()->Result<(),JxlEncodeErrors> {
    let mut encoder = JxlSimpleEncoder::new(&[255,0,255,0], EncoderOptions::new(2,2,co));
    encoder.encode().unwrap();
}
```