#![allow(clippy::upper_case_acronyms)]

use wasm_bindgen::prelude::wasm_bindgen;
use zune_core::colorspace::ColorSpace;
use zune_image::codecs::SupportedDecoders;

/// A 1 to 1 mapping of supported colorspaces
/// but with the `wasm_bindgen` attribute.
#[wasm_bindgen]
pub enum WasmColorspace
{
    RGB,
    RGBA,
    YCbCr,
    Luma,
    LumaA,
    RGBX,
    YCCK,
    CYMK,
    Unknown
}

impl WasmColorspace
{
    pub fn from_colorspace(colorspace: ColorSpace) -> WasmColorspace
    {
        match colorspace
        {
            ColorSpace::RGB => Self::RGB,
            ColorSpace::RGBA => Self::RGBA,
            ColorSpace::YCbCr => Self::YCbCr,
            ColorSpace::Luma => Self::Luma,
            ColorSpace::LumaA => Self::LumaA,
            ColorSpace::YCCK => Self::YCCK,
            ColorSpace::CMYK => Self::CYMK,
            ColorSpace::Unknown => Self::Unknown,
            _ => panic!("Unknown new colorspace {:?}", colorspace)
        }
    }
}

/// A one-to-one mapping of the image formats currently supported by
/// the decoder but with a `wasm_bindgen` attribute
#[wasm_bindgen]
pub enum WasmImageDecodeFormats
{
    /// Fully complete
    Jpeg,
    /// Not yet complete
    Png,
    /// Fully complete
    PPM,
    /// Partial support
    PSD,
    /// Full support
    Farbfeld,
    /// Full support
    QOI,
    /// Any unknown format.
    Unknown
}

impl WasmImageDecodeFormats
{
    pub fn from_formats(format: SupportedDecoders) -> WasmImageDecodeFormats
    {
        match format
        {
            SupportedDecoders::Jpeg => Self::Jpeg,
            SupportedDecoders::Png => Self::Png,
            SupportedDecoders::PPM => Self::PPM,
            SupportedDecoders::PSD => Self::PSD,
            SupportedDecoders::Farbfeld => Self::Farbfeld,
            SupportedDecoders::QOI => Self::QOI,
            SupportedDecoders::Unknown => Self::Unknown
        }
    }
}
