# zune-image

This is pre alpha pre beta, think of it like a leaked code.

<small>How I think image libraries should be </small>

This contains a POC code of some image stuff, very much WIP and has too many things not working,
and some working.

It aims to have the following features

- Safety(unsafe is allowed, but used sparingly)
- Fast decode speeds(JPEG decode speeds to match libjpeg-turbo)
- Fast image operation speed(probably some of the fastest image operation algorithms out there)
- Use explicit SIMD where applicable(for speed cases)
- Otherwise, trust the compiler.
- Small dependency footprint.

## Why yet another image library

Rust already has a good image library i.e https://github.com/image-rs/image
and there is probably no reason to have this,

But I'll let the overall speed of operations(decoding, applying image operations like blurring) speak for itself when
compared to other implementations.

## Library organization

- `zune-bin`: Provides a simple cmd application you can use for simple image manipulations

- `zune-image_format`: Provides an image decoder(and sometimes encoder) for that specific image format
  e.g `zune-jpeg` has a jpeg decoder, `zune-ppm` has a ppm decoder.
  Each decoder is independent of the rest of the decoders so one can pick whatever suits them.
- `zune-imageprocs`: Image processing routines, raw processing routines like `brighten` operations that work on native
  rust types
  (`u8` & `u16`) and are independent of image format.
- `zune-image`: A combined library relying on decoders and encoders and image processing routines that brings together
  the whole set together.
  allows one to slice , dice and splice images and create whatever suits you.
- `zune-core`: Core routines required by(and shared with) multiple image formats.

## What you can currently do

| IMAGE | Decoder          | Encoder              |
|-------|------------------|----------------------|
| JPEG  | Full support     | None                 |
| PNG   | Partial          | None                 |
| PPM   | 8 and 16 bit     | 8 and 16 bit support |
| PAL   | None             | 8 and 16 bit support |
| PSD   | 8 and 16 bit RGB | None                 |

`zune -i ([img].jpg | [img].ppm) -o [img].ppm`

i.e decode a jpg image,apply operations and encode it into
a ppm image.

## Timeline

- Things that we'll work in 1-2 months
    - PNG decoding
    - Image resizing
    - PAM decoding
    - Gaussian and box blurring
    - Unsharpen
- Things that will work in 2-4 months
    - JPEG encoding
    - PNG encoding
    - Blend modes
    - Edge detection
    - Erode
    - Sobel