/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use pyo3::pyclass;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace as ZColorSpace;
use zune_image::codecs::ImageFormat as ZImageFormat;
use zune_image::errors::ImageErrors;
use zune_imageprocs::threshold::ThresholdMethod;

#[pyclass]
pub struct ZImageErrors {
    pub(crate) error: zune_image::errors::ImageErrors
}

impl From<ImageErrors> for ZImageErrors {
    fn from(value: ImageErrors) -> Self {
        ZImageErrors { error: value }
    }
}

#[pyclass]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone)]
pub enum ImageFormat {
    PNG,
    JPEG,
    BMP,
    PPM,
    PSD,
    FarbFeld,
    Qoi,
    JPEG_XL,
    HDR,
    Unknown
}

impl ImageFormat {
    pub fn to_imageformat(self) -> ZImageFormat {
        match self {
            ImageFormat::PNG => ZImageFormat::PNG,
            ImageFormat::JPEG => ZImageFormat::JPEG,
            ImageFormat::BMP => ZImageFormat::BMP,
            ImageFormat::PPM => ZImageFormat::PPM,
            ImageFormat::PSD => ZImageFormat::PSD,
            ImageFormat::FarbFeld => ZImageFormat::Farbfeld,
            ImageFormat::Qoi => ZImageFormat::QOI,
            ImageFormat::JPEG_XL => ZImageFormat::JPEG_XL,
            ImageFormat::HDR => ZImageFormat::HDR,
            ImageFormat::Unknown => ZImageFormat::Unknown
        }
    }
    /// Return true if an image format has an encoder
    /// otherwise return false
    pub fn has_encoder(self) -> bool {
        self.to_imageformat().has_encoder()
    }
    /// Return true if an image format has a decoder
    /// otherwise return false
    pub fn has_decoder(self) -> bool {
        self.to_imageformat().has_decoder()
    }
}

impl From<ZImageFormat> for ImageFormat {
    fn from(value: ZImageFormat) -> Self {
        match value {
            ZImageFormat::JPEG => ImageFormat::JPEG,
            ZImageFormat::PNG => ImageFormat::PNG,
            ZImageFormat::PPM => ImageFormat::PPM,
            ZImageFormat::PSD => ImageFormat::PSD,
            ZImageFormat::Farbfeld => ImageFormat::FarbFeld,
            ZImageFormat::QOI => ImageFormat::Qoi,
            ZImageFormat::JPEG_XL => ImageFormat::JPEG_XL,
            ZImageFormat::HDR => ImageFormat::HDR,
            ZImageFormat::BMP => ImageFormat::BMP,
            _ => ImageFormat::Unknown
        }
    }
}

#[pyclass]
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone)]
pub enum ColorSpace {
    RGB,
    RGBA,
    Luma,
    LumaA,
    Unexposed,
    YCbCr,
    BGR,
    BGRA,
    CMYK,
    Unknown,
    HSL,
    HSV
}

impl ColorSpace {
    pub(crate) fn to_colorspace(self) -> ZColorSpace {
        match self {
            ColorSpace::RGB => ZColorSpace::RGB,
            ColorSpace::RGBA => ZColorSpace::RGBA,
            ColorSpace::Luma => ZColorSpace::Luma,
            ColorSpace::LumaA => ZColorSpace::LumaA,
            ColorSpace::YCbCr => ZColorSpace::YCbCr,
            ColorSpace::BGR => ZColorSpace::BGR,
            ColorSpace::BGRA => ZColorSpace::BGRA,
            ColorSpace::CMYK => ZColorSpace::CMYK,
            ColorSpace::HSL => ZColorSpace::HSL,
            ColorSpace::HSV => ZColorSpace::HSV,
            ColorSpace::Unexposed | ColorSpace::Unknown => ZColorSpace::Unknown
        }
    }
}

impl From<ZColorSpace> for ColorSpace {
    fn from(value: ZColorSpace) -> Self {
        match value {
            ZColorSpace::RGB => ColorSpace::RGB,
            ZColorSpace::RGBA => ColorSpace::RGBA,
            ZColorSpace::YCbCr => ColorSpace::YCbCr,
            ZColorSpace::Luma => ColorSpace::Luma,
            ZColorSpace::LumaA => ColorSpace::LumaA,
            ZColorSpace::YCCK => ColorSpace::Unexposed,
            ZColorSpace::CMYK => ColorSpace::CMYK,
            ZColorSpace::BGR => ColorSpace::BGR,
            ZColorSpace::BGRA => ColorSpace::BGRA,
            ZColorSpace::HSL => ColorSpace::HSL,
            ZColorSpace::HSV => ColorSpace::HSV,
            _ => ColorSpace::Unknown
        }
    }
}

#[pyclass]
#[derive(Copy, Clone, Debug)]
pub enum ImageDepth {
    U8,
    U16,
    F32,
    Unknown
}

impl ImageDepth {
    pub(crate) fn to_depth(self) -> BitDepth {
        match self {
            ImageDepth::U8 => BitDepth::Eight,
            ImageDepth::U16 => BitDepth::Sixteen,
            ImageDepth::F32 => BitDepth::Float32,
            ImageDepth::Unknown => BitDepth::Unknown
        }
    }
}

impl From<BitDepth> for ImageDepth {
    fn from(value: BitDepth) -> Self {
        match value {
            BitDepth::Eight => ImageDepth::U8,
            BitDepth::Sixteen => ImageDepth::U16,
            BitDepth::Float32 => ImageDepth::F32,
            _ => ImageDepth::Unknown
        }
    }
}

/// Different threshold arguments for the threshold parameter
#[pyclass]
#[derive(Copy, Clone, Debug)]
pub enum ImageThresholdType {
    Binary,
    BinaryInv,
    ThreshTrunc,
    ThreshToZero
}

impl ImageThresholdType {
    pub(crate) fn to_threshold(self) -> ThresholdMethod {
        match self {
            ImageThresholdType::Binary => ThresholdMethod::Binary,
            ImageThresholdType::BinaryInv => ThresholdMethod::BinaryInv,
            ImageThresholdType::ThreshTrunc => ThresholdMethod::ThreshTrunc,
            ImageThresholdType::ThreshToZero => ThresholdMethod::ThreshToZero
        }
    }
}
