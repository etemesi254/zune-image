/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use pyo3::pyclass;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_image::codecs::ImageFormat;
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
#[derive(Copy, Clone)]
pub enum ZImageFormats {
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

impl ZImageFormats {
    pub fn to_imageformat(self) -> ImageFormat {
        match self {
            ZImageFormats::PNG => ImageFormat::PNG,
            ZImageFormats::JPEG => ImageFormat::JPEG,
            ZImageFormats::BMP => ImageFormat::BMP,
            ZImageFormats::PPM => ImageFormat::PPM,
            ZImageFormats::PSD => ImageFormat::PSD,
            ZImageFormats::FarbFeld => ImageFormat::Farbfeld,
            ZImageFormats::Qoi => ImageFormat::QOI,
            ZImageFormats::JPEG_XL => ImageFormat::JPEG_XL,
            ZImageFormats::HDR => ImageFormat::HDR,
            ZImageFormats::Unknown => ImageFormat::Unknown
        }
    }
    /// Return true if an image format has an encoder
    /// otherwise return false
    pub fn has_encoder(&self) -> bool {
        self.to_imageformat().has_encoder()
    }
    /// Return true if an image format has a decoder
    /// otherwise return false
    pub fn has_decoder(&self) -> bool {
        self.to_imageformat().has_decoder()
    }
}

impl From<ImageFormat> for ZImageFormats {
    fn from(value: ImageFormat) -> Self {
        return match value {
            ImageFormat::JPEG => ZImageFormats::JPEG,
            ImageFormat::PNG => ZImageFormats::PNG,
            ImageFormat::PPM => ZImageFormats::PPM,
            ImageFormat::PSD => ZImageFormats::PSD,
            ImageFormat::Farbfeld => ZImageFormats::FarbFeld,
            ImageFormat::QOI => ZImageFormats::Qoi,
            ImageFormat::JPEG_XL => ZImageFormats::JPEG_XL,
            ImageFormat::HDR => ZImageFormats::HDR,
            ImageFormat::BMP => ZImageFormats::BMP,
            ImageFormat::Unknown => ZImageFormats::Unknown,
            _ => ZImageFormats::Unknown
        };
    }
}

#[pyclass]
#[derive(Copy, Clone)]
pub enum ZImageColorSpace {
    RGB,
    RGBA,
    Luma,
    LumaA,
    Unexposed,
    YCbCr,
    BGR,
    BGRA,
    CMYK,
    Unknown
}

impl ZImageColorSpace {
    pub(crate) fn to_colorspace(self) -> ColorSpace {
        match self {
            ZImageColorSpace::RGB => ColorSpace::RGB,
            ZImageColorSpace::RGBA => ColorSpace::RGBA,
            ZImageColorSpace::Luma => ColorSpace::Luma,
            ZImageColorSpace::LumaA => ColorSpace::LumaA,
            ZImageColorSpace::Unexposed => ColorSpace::Unknown,
            ZImageColorSpace::YCbCr => ColorSpace::YCbCr,
            ZImageColorSpace::BGR => ColorSpace::BGR,
            ZImageColorSpace::BGRA => ColorSpace::BGRA,
            ZImageColorSpace::CMYK => ColorSpace::CMYK,
            ZImageColorSpace::Unknown => ColorSpace::Unknown
        }
    }
}

impl From<ColorSpace> for ZImageColorSpace {
    fn from(value: ColorSpace) -> Self {
        return match value {
            ColorSpace::RGB => ZImageColorSpace::RGB,
            ColorSpace::RGBA => ZImageColorSpace::RGBA,
            ColorSpace::YCbCr => ZImageColorSpace::YCbCr,
            ColorSpace::Luma => ZImageColorSpace::Luma,
            ColorSpace::LumaA => ZImageColorSpace::LumaA,
            ColorSpace::YCCK => ZImageColorSpace::Unexposed,
            ColorSpace::CMYK => ZImageColorSpace::CMYK,
            ColorSpace::BGR => ZImageColorSpace::BGR,
            ColorSpace::BGRA => ZImageColorSpace::BGRA,
            ColorSpace::Unknown => ZImageColorSpace::Unknown,
            _ => ZImageColorSpace::Unknown
        };
    }
}

#[pyclass]
#[derive(Copy, Clone, Debug)]
pub enum ZImageDepth {
    U8,
    U16,
    F32,
    Unknown
}

impl ZImageDepth {
    pub(crate) fn to_depth(self) -> BitDepth {
        match self {
            ZImageDepth::U8 => BitDepth::Eight,
            ZImageDepth::U16 => BitDepth::Sixteen,
            ZImageDepth::F32 => BitDepth::Float32,
            ZImageDepth::Unknown => BitDepth::Unknown
        }
    }
}

impl From<BitDepth> for ZImageDepth {
    fn from(value: BitDepth) -> Self {
        match value {
            BitDepth::Eight => ZImageDepth::U8,
            BitDepth::Sixteen => ZImageDepth::U16,
            BitDepth::Float32 => ZImageDepth::F32,
            BitDepth::Unknown => ZImageDepth::Unknown,
            _ => ZImageDepth::Unknown
        }
    }
}

/// Different threshold arguments for the threshold parameter
#[pyclass]
#[derive(Copy, Clone, Debug)]
pub enum ZImageThresholdType {
    Binary,
    BinaryInv,
    ThreshTrunc,
    ThreshToZero
}

impl ZImageThresholdType {
    pub(crate) fn to_threshold(self) -> ThresholdMethod {
        match self {
            ZImageThresholdType::Binary => ThresholdMethod::Binary,
            ZImageThresholdType::BinaryInv => ThresholdMethod::BinaryInv,
            ZImageThresholdType::ThreshTrunc => ThresholdMethod::ThreshTrunc,
            ZImageThresholdType::ThreshToZero => ThresholdMethod::ThreshToZero
        }
    }
}
