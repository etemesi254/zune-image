## Zune-png

A fast, correct, and safe PNG and APNG decoder.

## Limitations

- This decoder (currently) expands images with less than 8 bpp to be 8 bits (one byte) automatically. This may or may not be desired depending on your use cases.

## Features

- Fast deflate decoder
- Vectorized filters and bit manipulation
- Memory friendly (few allocations)
- Support for both 8-bit and 16-bit image depths
- **Full support for Animated PNG (APNG) decoding**, including accurate frame disposal, alpha blending, and background compositing.

## Usage

First, include this in your `Cargo.toml`:

```toml
[dependencies]
zune-png = "0.5" # Make sure to check for the latest version
```

## Decoding 

### Static Images
For standard, single-frame PNGs, decoding is as simple as initializing the decoder and calling decode():

```rust
use zune_png::PngDecoder;
use zune_core::bytestream::ZCursor;

fn decode_png() {
    // Decode bytes
    let mut decoder = PngDecoder::new(ZCursor::new(b"bytes"));
    let pixels = decoder.decode().unwrap();
}
```

### Animated PNG
Decoding animated PNGs requires a loop to process frames sequentially. 
zune-png provides the `post_process_image_apng` utility to handle the complex state machine of frame disposal and alpha blending for you.

```rust
use zune_core::bytestream::ZCursor;
use zune_core::result::DecodingResult;
use zune_png::{PngDecoder, FrameInfo, ApngContext};

fn decode_apng() {
    let file_data = std::fs::read("animated.png").unwrap();
    let mut decoder = PngDecoder::new(ZCursor::new(&file_data));

    decoder.decode_headers().unwrap();
    
    // Get useful information about the image
    let colorspace = decoder.colorspace().unwrap();
    let info = decoder.info().unwrap().clone();

    // Allocate the main canvas
    let buffer_size = info.width * info.height * colorspace.num_components();
    let mut output = vec![0; buffer_size];

    // Initialize the APNG context BEFORE the loop.
    // This handles the backup canvas and gamma table internally.
    let mut ctx = ApngContext::<u8>::new(&info, colorspace, info.gamma);

    while decoder.more_frames() {
        decoder.decode_headers().unwrap();

        let frame = decoder.frame_info().unwrap().clone();
        let pix = decoder.decode_raw().unwrap();

        // Process the frame. The context automatically handles disposal of the
        // previous frame and state tracking!
        ctx.process_frame(&frame, &pix, &mut output).unwrap();

        // The `output` buffer now contains the fully composited frame.
        // e.g you can encode each frame separately
    }
}
```
## Debug vs release

The decoder heavily relies on platform specific intrinsics, namely SSE and NEON to gain speed-ups in decoding,
but they [perform poorly](https://godbolt.org/z/vPq57z13b) in debug builds. To get reasonable performance even
when compiling your program in debug mode, add this to your `Cargo.toml`:

```toml
# `zune-png` package will be always built with optimizations
[profile.dev.package.zune-png]
opt-level = 3
```

