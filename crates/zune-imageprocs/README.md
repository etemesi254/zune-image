## zune-imageprocs
A library for low level image processing routines

They work on raw pixels (`T`) and they are focused on speed and safety.

## Warning
Some filters are in alpha stage, and some are broken, 
don't use a filter with a `Broken` tag


## Usage
Add the crate to your dependencies e.g `cargo add zune-imageprocs`

After that one can use the processing routines since they implement `zune-image` `OperationsTrait`, anywhere that supports them
can call on them

E.g. to increase the exposure of an image

```rust
use zune_core::colorspace::ColorSpace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;
use zune_imageprocs::exposure::Exposure;

fn main()->Result<(),ImageErrors> {
    // create a 100x100 grayscale image
    let mut img = Image::from_fn::<u16, _>(100, 100, ColorSpace::Luma, |x, y, pix| {
        pix[0] = ((x + y) % 65536) as u16;
    });
    // increase each pixels strength by 2
    Exposure::new(2.0, 0.0).execute(&mut img)?;
    // write to a file
    img.save("hello.png")?;
}
```

### Features

- `portable-simd`: This adds support for [portable simd](https://github.com/rust-lang/portable-simd), requires a nightly compiler.
Some routines are written in portable simd and hence this can speed up some computations.
  - Disabled by default

- `threads`: Adds support for multithreading on image filters, some filters can run independently per channel, 
especially computational heavy filters, this enables that. On platforms without multithtreading (e.g `wasm`), this should
be disabled
  - Enabled by default

### Benchmarking 

Most routines in the library can be benchmarked, 
but they require a nightly compiler


To test speed of most routines run

```shell
cargo bench --features=benchmarks
```
