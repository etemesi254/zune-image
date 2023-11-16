## zune-psd

A simple photoshop reader. 

This crate doesn't handle any fancy photoshop features, including layering, blending,metadata extraction
and such, it simply copies some bytes it believes are the base layer hence it may not suit your needs

### Usage
1. First include it into your `Cargo.toml`

```shell
cargo add zune-psd
```

or include it directory in your `Cargo.toml`
```toml
[dependencies]
zune-psd="0.4"
```
Then use either one of the `decode_` variants to get pixel data
`decode_raw` will always return `Vec<u8>` while `decode` distinguishes return type via image
depth (either 8-bit or 16-bit)

### Speed
The decoder is fairly fast, we don't do any fancy processing so there is no need to compare it with other crates
(I'm not sure any supports full parsing), hence there are no benchmarks.

### Security
The crate is fuzz tested in CI to ensure untrusted input does not cause a panic