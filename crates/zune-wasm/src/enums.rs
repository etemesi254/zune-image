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
use zune_imageprocs::color_transform::ColorProfiles;
use zune_imageprocs::pad::PadMethod;
use zune_imageprocs::resize::ResizeMethod;
use zune_imageprocs::spatial_ops::SpatialOperations;
use zune_imageprocs::composite::CompositeMethod;


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
    HSV,
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
            e => panic!("Unknown colorspace {e:?}"),
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
            WasmColorspace::HSV => ColorSpace::HSV,
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
    /// Heic images
    HEIC,
    /// WEBP Image
    WEBP,
    /// JPEG 2000 image
    JPEG_2000,
    /// Any unknown format.
    Unknown,
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
            ImageFormat::WEBP => Self::WEBP,
            ImageFormat::HEIC => Self::HEIC,
            ImageFormat::JPEG_2000 => Self::JPEG_2000,
            ImageFormat::Custom(_) => {
                todo!("custom image formats is not yet implemented");
            }
            _ => {
                todo!("Not added yey")
            }
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
            WasmImageFormats::Unknown => ImageFormat::Unknown,
            WasmImageFormats::HEIC => ImageFormat::HEIC,
            WasmImageFormats::WEBP => ImageFormat::WEBP,
            WasmImageFormats::JPEG_2000 => ImageFormat::JPEG_2000,
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
    Mean,
}
impl From<SpatialOperations> for WasmSpatialOperations {
    fn from(value: SpatialOperations) -> Self {
        match value {
            SpatialOperations::Contrast => WasmSpatialOperations::Contrast,
            SpatialOperations::Maximum => WasmSpatialOperations::Maximum,
            SpatialOperations::Gradient => WasmSpatialOperations::Gradient,
            SpatialOperations::Minimum => WasmSpatialOperations::Minimum,
            SpatialOperations::Mean => WasmSpatialOperations::Mean,
        }
    }
}
impl From<WasmSpatialOperations> for SpatialOperations {
    fn from(val: WasmSpatialOperations) -> Self {
        match val {
            WasmSpatialOperations::Contrast => SpatialOperations::Contrast,
            WasmSpatialOperations::Maximum => SpatialOperations::Maximum,
            WasmSpatialOperations::Gradient => SpatialOperations::Gradient,
            WasmSpatialOperations::Minimum => SpatialOperations::Minimum,
            WasmSpatialOperations::Mean => SpatialOperations::Mean,
        }
    }
}

#[wasm_bindgen(js_name=PadMethod)]
pub enum WasmPadMethod {
    Constant,
    Replicate,
}
impl From<PadMethod> for WasmPadMethod {
    fn from(value: PadMethod) -> Self {
        match value {
            PadMethod::Constant => WasmPadMethod::Constant,
            PadMethod::Replicate => WasmPadMethod::Replicate,
        }
    }
}

#[wasm_bindgen(js_name=ColorProfiles)]
pub enum WasmColorProfiles {
    sRGB,
    AdobeRgb,
    DisplayP3,
    Bt2020,
    DciP3,
}
impl From<ColorProfiles> for WasmColorProfiles {
    fn from(value: ColorProfiles) -> Self {
        match value {
            ColorProfiles::sRGB => WasmColorProfiles::sRGB,
            ColorProfiles::AdobeRgb => WasmColorProfiles::AdobeRgb,
            ColorProfiles::DisplayP3 => WasmColorProfiles::DisplayP3,
            ColorProfiles::Bt2020 => WasmColorProfiles::Bt2020,
            ColorProfiles::DciP3 => WasmColorProfiles::DciP3,
        }
    }
}
impl From<WasmColorProfiles> for ColorProfiles {
    fn from(val: WasmColorProfiles) -> Self {
        match val {
            WasmColorProfiles::sRGB => ColorProfiles::sRGB,
            WasmColorProfiles::AdobeRgb => ColorProfiles::AdobeRgb,
            WasmColorProfiles::DisplayP3 => ColorProfiles::DisplayP3,
            WasmColorProfiles::Bt2020 => ColorProfiles::Bt2020,
            WasmColorProfiles::DciP3 => ColorProfiles::DciP3,
        }
    }
}

#[wasm_bindgen(js_name=ResizeMethod)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WasmResizeMethod {
    /// Lanczos filter with a window of 3. Provides the highest quality and sharpest
    /// results for both upscaling and downscaling, but is the slowest.
    Lanczos3,
    /// Lanczos filter with a window of 2. A slightly faster, slightly softer alternative to Lanczos3.
    Lanczos2,
    /// Bicubic interpolation (Mitchell-Netravali). A good balance of speed and quality.
    Bicubic,
    /// Catmull-Rom spline. Produces sharp edges without the ringing artifacts sometimes seen in Lanczos.
    CatmullRom,
    /// Mitchell filter. An alias for Bicubic interpolation.
    Mitchell,
    /// B-Spline interpolation. Produces very smooth/soft results.
    BSpline,
    /// Hermite filter. Fast, but relatively soft.
    Hermite,
    /// Sinc filter with a window radius of 3.
    Sinc,
    /// Bilinear interpolation. Very fast, but produces blurry results when upscaling
    /// and aliasing artifacts when downscaling.
    Bilinear,
}

impl Into<ResizeMethod> for WasmResizeMethod {
    fn into(self) -> ResizeMethod {
        match self {
            WasmResizeMethod::Lanczos3 => ResizeMethod::Lanczos3,
            WasmResizeMethod::Lanczos2 => ResizeMethod::Lanczos2,
            WasmResizeMethod::Bicubic => ResizeMethod::Bicubic,
            WasmResizeMethod::CatmullRom => ResizeMethod::CatmullRom,
            WasmResizeMethod::Mitchell => ResizeMethod::Mitchell,
            WasmResizeMethod::BSpline => ResizeMethod::BSpline,
            WasmResizeMethod::Hermite => ResizeMethod::Hermite,
            WasmResizeMethod::Sinc => ResizeMethod::Sinc,
            WasmResizeMethod::Bilinear => ResizeMethod::Bilinear,
        }
    }
}

#[wasm_bindgen(js_name=FlipDirection)]
pub enum WasmFlipDirection {
    Horizontal,
    Vertical,
}

#[wasm_bindgen(js_name=MirrorMode)]
pub enum WasmMirrorMode {
    Horizontal,
    Vertical,
}

#[wasm_bindgen(js_name=ThresholdMethod)]
pub enum WasmThresholdMethod {
    /// If the pixel is greater than the threshold, it is set to the maximum value.
    /// Otherwise, it is set to 0.
    Binary,
    /// If the pixel is greater than the threshold, it is set to 0.
    /// Otherwise, it is set to the maximum value.
    BinaryInv,
    /// If the pixel is greater than the threshold, it is truncated to exactly the threshold value.
    /// Otherwise, it remains unchanged.
    ThreshTrunc,
    /// If the pixel is greater than the threshold, it remains unchanged.
    /// Otherwise, it is set to 0.
    ThreshToZero,
}


#[wasm_bindgen]
#[derive(Copy, Clone, Debug)]
pub enum WasmCompositeMethod {
    Over,
    Src,
    Dst,
    DstIn,
    DstOut,
    SrcIn,
    SrcOut,
    Xor,
    Multiply,
    Screen,
}

impl From<WasmCompositeMethod> for CompositeMethod {
    fn from(val: WasmCompositeMethod) -> Self {
        match val {
            WasmCompositeMethod::Over => CompositeMethod::Over,
            WasmCompositeMethod::Src => CompositeMethod::Src,
            WasmCompositeMethod::Dst => CompositeMethod::Dst,
            WasmCompositeMethod::DstIn => CompositeMethod::DstIn,
            WasmCompositeMethod::DstOut => CompositeMethod::DstOut,
            WasmCompositeMethod::SrcIn => CompositeMethod::SrcIn,
            WasmCompositeMethod::SrcOut => CompositeMethod::SrcOut,
            WasmCompositeMethod::Xor => CompositeMethod::Xor,
            WasmCompositeMethod::Multiply => CompositeMethod::Multiply,
            WasmCompositeMethod::Screen => CompositeMethod::Screen,
        }
    }
}