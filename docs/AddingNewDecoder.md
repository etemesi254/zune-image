# Adding a new decoder

## If it's an external decoder.

Just hook it up and define `DecoderTrait` and watch everything fall into place

If the format returns `&[u8]` bytes as decoded results,
use `Image::from_u8`.

Remember to match colorspace information from image to the mapping this image understands

See the example below for decoders distributed inside `zune` especially the third step.

## If it is to be distributed inside `zune`

This one takes some time

Image decoders usually gets an array of  `[u8]` from the decoder
from which it is supposed to return an `image`

I'm going to talk about adding a format such as `jpeg`
and I'll use the `mozjpeg` crate

So hypothetically speaking, if the library were to add
`jpeg` support via that `mozjpeg` crate the following steps would occur

### 1. Add it to Cargo.toml

```toml
mozjpeg = "0.9.4"
```

### 2. Create a new file in `image/src/codecs/{FORMAT_NAME}.rs`

For our case `{FORMAT_NAME}` is `jpeg`

This will hold the decoder entry and will be responsible for translating decoded bytes `&[u8]`
to an `Image` struct.

### 3. Implement `DecoderTrait` for `mozjpeg`

`DecoderTrait` is a format antagonistic trait that implements an image decoder, when called the raw encoded bytes are
decoded into a respective image

After decoding the image, we need extra values from the image to correctly use it,
we need it's colorspace,image dimensions and its depth(for images with support for various bit depths)

Fortunately `mozjpeg` crate provides this details for us after decoding

So the following pseudocode works for us

```Rust
// not tested, probably doesn't work
use mozjpeg;

impl<'a> DecoderTrait<'a> for mozjpeg::Decompress<'a> {
    fn decode(&mut self) -> Image {
        // assume pixels contain our data
        let pixels = self.rgb().read_scanlines();
        // width and height are given here
        let width = self.width();
        let height = self.height();
        // colorspace is rgb becuae we called `self.rgb()` up
        // there
        Image::from_u8(pixels, width, height, ColorSpace::RGB);
    }
    // remember to implement other methods
}
```

And that's the hard part, the rest just add compatibility and enable other parts to
pick up on formats(e.g the binary)

### Add an enum variant for format

If the image is a new type, you'll need to add an enum variant that represents that image
format. i.e if the image adds `webp` support, you need to add `WEBP` to `zune-image/src/image_format`

You also need to provide magic bytes which will be used to identify this image.

Furthermore, you'll need to fill in values in the methods of `ImageFormat`, e.g whether the format
has an encoder present, and whether it has a decoder present, and provide image decoder and encoder that
achieves this as a `Box<dyn DecoderTrait<'a>+'a>`
