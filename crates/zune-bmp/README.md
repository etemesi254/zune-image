## zune-bmp

A lean, mean and green BMP decoder.

This crate contains a fast implemtnation of a BMP decoder
with battery included support for the esoteric parts of the spec

### Features
- RLE support
- 1-bit,4-bit,8-bit,16-bit,24-bit and 32-bit support
- Performant

### Usage
First add the project to your library/binary

```toml
zune-bmp = "0.4" # Or use cargo add zune-bmp
```

Then you can toy with the other configs


```rust
use zune_bmp::BmpDecoder;
use zune_bmp::BmpDecoderErrors;

fn main()->Result<(),BmpDecodeErrors>{
    let decoder:Vec<u8> = BmpDecoder::new(b"BMP").decode()?;

}

```

### Security
The decoder is continuously fuzz tested in CI to ensure it does not crash on malicious input in case a sample causes it to crash, an issue would be welcome.