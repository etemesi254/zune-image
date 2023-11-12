# zune-hdr

A fast,small versatile hdr decoder and encoder

This crate contains a small and fast Radiance (.hdr) decoder and encoder.

## Usage

To use this crate, add `zune-hdr` to your  `Cargo.toml` or run `cargo add zune-hdr`

Here is an example of loading a hdr image

```rust
use std::error::Error;
use std::fs::read;
use zune_hdr::HdrDeocder;

fn main() -> Result<(), Box<dyn Error>> {
    let contents = read("file.hdr")?;
    let data = HdrDecoder::new(contents);
    let pix: Vec<f32> = data.decode()?;
    println!("first pix:{}", pix[0]);
}
```

Here is an example of writing a hdr, this generates a black image

```rust
use zune_hdr::HdrEncoder;
use zune_core::options::EncoderOptions;

fn main() -> Result<(), Box<dyn Error>> {
    let w = 100;
    let h = 100;
    let comp = 3;

    let in_size: Vec<f32> = vec![0.0; w * h * comp];
    // setup options, we specify width and height here which are needed, colorspace must always 
    // be rgb and the depth is f32, 
    let encoder_opts = EncoderOptions::new(w, h, ColorSpace::RGB, BitDepth::Float32);

    let encoder = HdrEncoder::new(&in_size, encoder_opts);
    encoder.encode()?;
}
```

## Performance

The crate boasts an optimized decoder, with it being about 2.5x faster than `image-rs/hdr` decoder,
the following is a benchmark run on the
following [image](https://github.com/etemesi254/zune-image/blob/dev/test-images/hdr/memorial.hdr)

This can be replicated with

```shell
git clone
cd ./zune-image 
cargo bench --workspace "hdr"
```

### Formatted output
| field | image-rs/hdr | zune-hdr     |
|-------|--------------|--------------|
| time  | 16.561 ms    | 6.4180 ms    |
| thrpt | 77.363 MiB/s | 199.63 MiB/s |
  
### Raw  criterion output

```text
Running benches/decode_hdr.rs (deps/decode_hdr-b0d728bd626a2ee2)
hdr: Simple decode(memorial-hdr)/image-rs/hdr
time:   [16.542 ms 16.561 ms 16.581 ms]
thrpt:  [77.271 MiB/s 77.363 MiB/s 77.454 MiB/s]

hdr: Simple decode(memorial-hdr)/zune-image/hdr
time:   [6.3522 ms 6.4180 ms 6.4848 ms]
thrpt:  [197.58 MiB/s 199.63 MiB/s 201.70 MiB/s]
```

## Security

The crate has been extensively fuzzed, additionaly the CI does fuzz testing every day
to catch unintended bugs

The crate does not use `unsafe` and uses `#[forbid(unsafe)]` to prevent any from creeping in
