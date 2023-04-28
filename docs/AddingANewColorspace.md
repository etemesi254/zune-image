## Adding a new colorspace

To add a new colorspace to the library, the following steps are need

We are going to assume that we are trying to add the colorspace `XYZ`

### 1. Add enum variant to  `core/src/colorspace.rs/ColorSpace`

```Rust
pub enum ColorSpace {
    //.. other definitions
    RGB,
    // the new colorspace we are trying to add
    XYZ,
}
```

### 2. Add methods needed for enums

- `XYZ` colorspace has 3 components, and no alpha channel, so we add it's definition in `num_components` method as
  returning
  three components

```Rust
impl ColorSpace {
    pub fn num_components(self) {
        match self
        {
            Self::RGB | Self::YCbCr | Self::BGR | Self::XYZ /*new definition*/ => 3,
            Self::RGBA | Self::YCCK | Self::CMYK | Self::BGRA => 4,
            Self::Luma => 1,
            Self::LumaA => 2,
            Self::Unknown => 0
        }
    }
}
```

### 3. Add the new colorspace to the `ALL_COLORSPACES` trait

```Rust
pub static ALL_COLORSPACES: [ColorSpace; 10] = [
    // .. PREVIOUS DEFINITIONS
    ColorSpace::YCbCr,
    ColorSpace::XYZ // new definition
];
```

### 4. (Optional) add a mapping from the colorspace to RGB and back

It is recommended to add a mapping that converts the colorspace from RGB to it and vice versa, as most
decoders and encoders understand RGB.

This is done in the `zune-image/src/core_filters/colorspace.rs` file