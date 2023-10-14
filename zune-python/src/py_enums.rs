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
use zune_image::filters::threshold::ThresholdMethod;

#[pyclass]
pub struct PyImageErrors {
    pub(crate) error: zune_image::errors::ImageErrors
}

impl From<ImageErrors> for PyImageErrors {
    fn from(value: ImageErrors) -> Self {
        PyImageErrors { error: value }
    }
}

#[pyclass]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum PyImageFormats {
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

impl PyImageFormats {
    pub fn to_imageformat(self) -> ImageFormat {
        match self {
            PyImageFormats::PNG => ImageFormat::PNG,
            PyImageFormats::JPEG => ImageFormat::JPEG,
            PyImageFormats::BMP => ImageFormat::BMP,
            PyImageFormats::PPM => ImageFormat::PPM,
            PyImageFormats::PSD => ImageFormat::PSD,
            PyImageFormats::FarbFeld => ImageFormat::Farbfeld,
            PyImageFormats::Qoi => ImageFormat::QOI,
            PyImageFormats::JPEG_XL => ImageFormat::JPEG_XL,
            PyImageFormats::HDR => ImageFormat::HDR,
            PyImageFormats::Unknown => ImageFormat::Unknown
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

impl From<ImageFormat> for PyImageFormats {
    fn from(value: ImageFormat) -> Self {
        return match value {
            ImageFormat::JPEG => PyImageFormats::JPEG,
            ImageFormat::PNG => PyImageFormats::PNG,
            ImageFormat::PPM => PyImageFormats::PPM,
            ImageFormat::PSD => PyImageFormats::PSD,
            ImageFormat::Farbfeld => PyImageFormats::FarbFeld,
            ImageFormat::QOI => PyImageFormats::Qoi,
            ImageFormat::JPEG_XL => PyImageFormats::JPEG_XL,
            ImageFormat::HDR => PyImageFormats::HDR,
            ImageFormat::BMP => PyImageFormats::BMP,
            ImageFormat::Unknown => PyImageFormats::Unknown,
            _ => PyImageFormats::Unknown
        };
    }
}

#[pyclass]
#[derive(Copy, Clone)]
pub enum PyImageColorSpace {
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

impl PyImageColorSpace {
    pub(crate) fn to_colorspace(self) -> ColorSpace {
        match self {
            PyImageColorSpace::RGB => ColorSpace::RGB,
            PyImageColorSpace::RGBA => ColorSpace::RGBA,
            PyImageColorSpace::Luma => ColorSpace::Luma,
            PyImageColorSpace::LumaA => ColorSpace::LumaA,
            PyImageColorSpace::Unexposed => ColorSpace::Unknown,
            PyImageColorSpace::YCbCr => ColorSpace::YCbCr,
            PyImageColorSpace::BGR => ColorSpace::BGR,
            PyImageColorSpace::BGRA => ColorSpace::BGRA,
            PyImageColorSpace::CMYK => ColorSpace::CMYK,
            PyImageColorSpace::Unknown => ColorSpace::Unknown
        }
    }
}

impl From<ColorSpace> for PyImageColorSpace {
    fn from(value: ColorSpace) -> Self {
        return match value {
            ColorSpace::RGB => PyImageColorSpace::RGB,
            ColorSpace::RGBA => PyImageColorSpace::RGBA,
            ColorSpace::YCbCr => PyImageColorSpace::YCbCr,
            ColorSpace::Luma => PyImageColorSpace::Luma,
            ColorSpace::LumaA => PyImageColorSpace::LumaA,
            ColorSpace::YCCK => PyImageColorSpace::Unexposed,
            ColorSpace::CMYK => PyImageColorSpace::CMYK,
            ColorSpace::BGR => PyImageColorSpace::BGR,
            ColorSpace::BGRA => PyImageColorSpace::BGRA,
            ColorSpace::Unknown => PyImageColorSpace::Unknown,
            _ => PyImageColorSpace::Unknown
        };
    }
}

#[pyclass]
#[derive(Copy, Clone, Debug)]
pub enum PyImageDepth {
    Eight,
    Sixteen,
    F32,
    Unknown
}

impl PyImageDepth {
    pub(crate) fn to_depth(self) -> BitDepth {
        match self {
            PyImageDepth::Eight => BitDepth::Eight,
            PyImageDepth::Sixteen => BitDepth::Sixteen,
            PyImageDepth::F32 => BitDepth::Float32,
            PyImageDepth::Unknown => BitDepth::Unknown
        }
    }
}

impl From<BitDepth> for PyImageDepth {
    fn from(value: BitDepth) -> Self {
        match value {
            BitDepth::Eight => PyImageDepth::Eight,
            BitDepth::Sixteen => PyImageDepth::Sixteen,
            BitDepth::Float32 => PyImageDepth::F32,
            BitDepth::Unknown => PyImageDepth::Unknown,
            _ => PyImageDepth::Unknown
        }
    }
}

/// Different threshold arguments for the threshold parameter
#[pyclass]
#[derive(Copy, Clone, Debug)]
pub enum PyImageThresholdType {
    Binary,
    BinaryInv,
    ThreshTrunc,
    ThreshToZero
}

impl PyImageThresholdType {
    pub(crate) fn to_threshold(self) -> ThresholdMethod {
        match self {
            PyImageThresholdType::Binary => ThresholdMethod::Binary,
            PyImageThresholdType::BinaryInv => ThresholdMethod::BinaryInv,
            PyImageThresholdType::ThreshTrunc => ThresholdMethod::ThreshTrunc,
            PyImageThresholdType::ThreshToZero => ThresholdMethod::ThreshToZero
        }
    }
}
