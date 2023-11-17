## Zune-image

An (opinionated) image library

This is the main library tying most of the `zune-` family of image 
decoders, encoders and image processors and image libraries


## Supported formats

| Format   | Library                      | Decoding | Encoding      |
|----------|------------------------------|----------|---------------|
| BMP      | [zune-bmp]                   | Yes      | -             |
| Farbfeld | [zune-farbfeld]              | Yes      | Yes           |
| HDR      | [zune-hdr]                   | Yes      | Yes           |
| JPEG     | [zune-jpeg] , [jpeg-encoder] | Yes      | Yes           |
| JPEG-XL  | [zune-jpegxl], [jxl-oxide]   | Yes      | Lossless only | 
| PNG      | [zune-png]                   | Yes      | Yes           |
| PPM      | [zune-ppm]                   | Yes      | Yes           |
| QOI      | [zune-qoi]                   | Yes      | Yes           |
 
[zune-bmp]:https://crates.io/crates/zune-bmp
[zune-farbfeld]:https://crates.io/crates/zune-farbfeld
[zune-hdr]: https://crates.io/crates/zune-hdr
[zune-jpeg]: https://crates.io/crates/zune-jpeg
[zune-png]: https://crates.io/crates/zune-png
[zune-ppm]: https://crates.io/crates/zune-ppm
[zune-qoi]: https://crates.io/crates/zune-qoi
[zune-jpegxl]: https://crates.io/crates/zune-jpegxl
[jpeg-encoder]: https://crates.io/crates/jpeg-encoder
[jxl-oxide]: https://crates.io/crates/jxl-oxide


## Features

### Image decoders and encoders
Each image decoder and encoder can be disabled or enabled by toggling it's feature, e.g to only include
jpeg decoding and encoding, one can use
```toml
zune-image={version="0.4",default-features=false,features=["jpeg"]}
```


### Other features 
- `serde`: Enable serde support for serializing image metadata, adds `serde` as a dependency
- `log`: Enable printing information via `log` crate
- `simd`: Enable SIMD support for certain image operations, 
- this just enables explicitly written simd code, not compiler autovectorization which may generate simd
- `threads`: Enables support for running some operations in multiple threads, if this is disabled, the library can run 
in areas which lack support for threading e.g `wasm`
- `image-formats`: Blanket feature to include all supported image formats
- `metadata`: Enable parsing of exif data,  adds `kamadak-exif` as a dependency
- `all`: Enables all the above features


## `DecoderTrait`, `OperationsTrait` and `EncoderTrait`

These traits encapsulate the main operations expected to be performed by an image library
- `DecoderTrait`: Any item implementing this can decode an image into the library's `Image` representation
- `OperationsTrait`: Any item implementing this can manipulate an `Image` modifying appropriate fields where necessary
- `EncoderTrait`: Any item implementing this can take an `Image` and encode it to a desired function

## Representation of images
All images are represented as an `Image` struct, this doesn't matter if the image is Grayscale, RGB, animated or represented
by `f32`, this allows easy interoperability and simpler api at the cost of a slightly complex internal API

You can create images via the `from_` methods or read an image file via `Image.open` 


## Examples
- Generating fractals, the same example as `image` crate

```rust
use zune_core::colorspace::ColorSpace;

fn main() {
    let img_x = 800;
    let img_y = 800;

    let scale_x = 3.0 / img_x as f32;
    let scale_y = 3.0 / img_y as f32;

    let mut image = zune_image::image::Image::from_fn(img_x, img_y, ColorSpace::RGB, |y, x, px| {
        let r = (0.3 * x as f32) as u8;
        let b = (0.3 * y as f32) as u8;

        // colorspace channels are three, so we must set three pixels
        px[0] = r;
        px[1] = 0;
        px[2] = b;
    });

    // This may actually be combined with the function `from_fn` above.
    // But it tries to match the image example given as much as possible
    //
    // we have to annotate our image is `u8` so that it works
    image
        .modify_pixels_mut::<u8, _>(|y, x, px| {
            let cx = y as f32 * scale_x - 1.5;
            let cy = x as f32 * scale_y - 1.5;

            let c = num_complex::Complex::new(-0.4, 0.6);
            let mut z = num_complex::Complex::new(cx, cy);

            let mut i = 0;
            while i < 255 && z.norm() <= 2.0 {
                z = z * z + c;
                i += 1;
            }
            // write it
            *px[1] = i as u8;
        })
        .unwrap();

    image.save("./fractals.jpg").unwrap();
}
```


