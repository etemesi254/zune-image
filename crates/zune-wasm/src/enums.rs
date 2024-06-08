/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(clippy::upper_case_acronyms, non_camel_case_types)]

use wasm_bindgen::prelude::wasm_bindgen;
use zune_core::colorspace::ColorSpace;
use zune_image::codecs::ImageFormat;
use zune_imageprocs::pad::PadMethod;
use zune_imageprocs::spatial_ops::SpatialOperations;

/// A 1 to 1 mapping of supported colorspaces
/// but with the `wasm_bindgen` attribute.
#[wasm_bindgen(js_name=Colorspace)]
pub enum WasmColorspace {
    RGB,
    RGBA,
    YCbCr,
    Luma,
    LumaA,
    YCCK,
    CYMK,
    Unknown,
    BGR,
    BGRA,
    ARGB,
    HSL,
    HSV
}

impl WasmColorspace {
    pub fn from_colorspace(colorspace: ColorSpace) -> WasmColorspace {
        match colorspace {
            ColorSpace::RGB => Self::RGB,
            ColorSpace::RGBA => Self::RGBA,
            ColorSpace::YCbCr => Self::YCbCr,
            ColorSpace::Luma => Self::Luma,
            ColorSpace::LumaA => Self::LumaA,
            ColorSpace::YCCK => Self::YCCK,
            ColorSpace::CMYK => Self::CYMK,
            ColorSpace::Unknown => Self::Unknown,
            ColorSpace::BGR => Self::BGR,
            ColorSpace::BGRA => Self::BGRA,
            ColorSpace::ARGB => Self::ARGB,
            ColorSpace::HSL => Self::HSL,
            ColorSpace::HSV => Self::HSV,
            e => panic!("Unknown colorspace {:?}", e)
        }
    }
    pub fn to_colorspace(&self) -> ColorSpace {
        match self {
            WasmColorspace::RGB => ColorSpace::RGB,
            WasmColorspace::RGBA => ColorSpace::RGBA,
            WasmColorspace::YCbCr => ColorSpace::YCbCr,
            WasmColorspace::Luma => ColorSpace::Luma,
            WasmColorspace::LumaA => ColorSpace::LumaA,
            WasmColorspace::YCCK => ColorSpace::YCCK,
            WasmColorspace::CYMK => ColorSpace::CMYK,
            WasmColorspace::Unknown => ColorSpace::Unknown,
            WasmColorspace::BGR => ColorSpace::BGR,
            WasmColorspace::BGRA => ColorSpace::BGRA,
            WasmColorspace::ARGB => ColorSpace::ARGB,
            WasmColorspace::HSL => ColorSpace::HSL,
            WasmColorspace::HSV => ColorSpace::HSV
        }
    }
}

/// A one-to-one mapping of the image formats currently supported by
/// the decoder but with a `wasm_bindgen` attribute
#[wasm_bindgen(js_name=ImageFormats)]
pub enum WasmImageFormats {
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
    /// Encoder and Decoder
    HDR,
    /// Losless Encoder and Decoder
    JPEG_XL,
    /// Decoder only (encoder in the works)
    BMP,

    /// Any unknown format.
    Unknown
}

impl WasmImageFormats {
    pub fn from_formats(format: ImageFormat) -> WasmImageFormats {
        match format {
            ImageFormat::JPEG => Self::Jpeg,
            ImageFormat::PNG => Self::Png,
            ImageFormat::PPM => Self::PPM,
            ImageFormat::PSD => Self::PSD,
            ImageFormat::Farbfeld => Self::Farbfeld,
            ImageFormat::QOI => Self::QOI,
            ImageFormat::Unknown => Self::Unknown,
            ImageFormat::JPEG_XL => Self::JPEG_XL,
            ImageFormat::HDR => Self::HDR,
            ImageFormat::BMP => Self::BMP,
            _ => todo!("Support format {:?}", format)
        }
    }
    pub fn to_format(&self) -> ImageFormat {
        match self {
            WasmImageFormats::Jpeg => ImageFormat::JPEG,
            WasmImageFormats::Png => ImageFormat::PNG,
            WasmImageFormats::PPM => ImageFormat::PPM,
            WasmImageFormats::PSD => ImageFormat::PSD,
            WasmImageFormats::Farbfeld => ImageFormat::Farbfeld,
            WasmImageFormats::QOI => ImageFormat::QOI,
            WasmImageFormats::HDR => ImageFormat::HDR,
            WasmImageFormats::JPEG_XL => ImageFormat::JPEG_XL,
            WasmImageFormats::BMP => ImageFormat::BMP,
            WasmImageFormats::Unknown => ImageFormat::Unknown
        }
    }
}

#[wasm_bindgen(js_name=SpatialOperations)]
pub enum WasmSpatialOperations {
    /// (max-min)/(max+min)
    Contrast,
    /// max
    Maximum,
    /// max-min
    Gradient,
    /// min
    Minimum,
    /// sum(pix)/len
    Mean
}
impl From<SpatialOperations> for WasmSpatialOperations {
    fn from(value: SpatialOperations) -> Self {
        match value {
            SpatialOperations::Contrast => WasmSpatialOperations::Contrast,
            SpatialOperations::Maximum => WasmSpatialOperations::Maximum,
            SpatialOperations::Gradient => WasmSpatialOperations::Gradient,
            SpatialOperations::Minimum => WasmSpatialOperations::Minimum,
            SpatialOperations::Mean => WasmSpatialOperations::Mean
        }
    }
}
impl Into<SpatialOperations> for WasmSpatialOperations {
    fn into(self) -> SpatialOperations {
        match self {
            WasmSpatialOperations::Contrast => SpatialOperations::Contrast,
            WasmSpatialOperations::Maximum => SpatialOperations::Maximum,
            WasmSpatialOperations::Gradient => SpatialOperations::Gradient,
            WasmSpatialOperations::Minimum => SpatialOperations::Minimum,
            WasmSpatialOperations::Mean => SpatialOperations::Mean
        }
    }
}

#[wasm_bindgen(js_name=PadMethod)]
pub enum WasmPadMethod {
    Constant,
    Replicate
}
impl From<PadMethod> for WasmPadMethod {
    fn from(value: PadMethod) -> Self {
        match value {
            PadMethod::Constant => WasmPadMethod::Constant,
            PadMethod::Replicate => WasmPadMethod::Replicate
        }
    }
}
