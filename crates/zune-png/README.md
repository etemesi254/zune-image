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
use zune_png::{PngDecoder, FrameInfo, post_process_image_apng};

fn decode_apng() {
    let file_data = std::fs::read("animated.png").unwrap();
    let mut decoder = PngDecoder::new(ZCursor::new(&file_data));

    decoder.decode_headers().unwrap();

    if decoder.is_animated() {
        let info = decoder.info().unwrap().clone();
        let colorspace = decoder.colorspace().unwrap();

        // Allocate the main canvas and a backup canvas for frame disposal
        let buffer_size = info.width * info.height * colorspace.num_components();
        let mut output_canvas = vec![0u8; buffer_size];
        let mut backup_canvas = vec![0u8; buffer_size];

        let mut prev_frame_info: Option<FrameInfo> = None;

        while decoder.more_frames() {
            decoder.decode_headers().unwrap();
            let frame = decoder.frame_info().unwrap().clone();

            if let DecodingResult::U8(frame_pixels) = decoder.decode().unwrap() {
                // Run the compositing lifecycle (Disposal -> Blending)
                post_process_image_apng(
                    &info,
                    colorspace,
                    &frame,
                    prev_frame_info.as_ref(),
                    &frame_pixels,
                    &mut backup_canvas,
                    &mut output_canvas,
                    None
                ).unwrap();

                // `output_canvas` now holds the fully rendered frame!
                // Do something with it (e.g., save or display) here.

                prev_frame_info = Some(frame);
            }
        }
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

