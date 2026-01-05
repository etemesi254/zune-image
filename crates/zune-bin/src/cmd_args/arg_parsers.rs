/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use clap::builder::PossibleValue;
use clap::ValueEnum;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::resize::ResizeMethod;

#[derive(Copy, Clone, Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum IColorSpace {
    RGB,
    GRAYSCALE,
    YCbCr,
    RGBA,
    Luma,
    LumaA
}

impl IColorSpace {
    #[allow(dead_code)]
    pub const fn to_colorspace(self) -> ColorSpace {
        match self {
            IColorSpace::RGB => ColorSpace::RGB,
            IColorSpace::GRAYSCALE => ColorSpace::Luma,
            IColorSpace::YCbCr => ColorSpace::YCbCr,
            IColorSpace::RGBA => ColorSpace::RGBA,
            IColorSpace::Luma => ColorSpace::Luma,
            IColorSpace::LumaA => ColorSpace::LumaA
        }
    }
}

impl ValueEnum for IColorSpace {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::RGBA,
            Self::RGB,
            Self::YCbCr,
            Self::GRAYSCALE,
            Self::Luma,
            Self::LumaA
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::RGBA => PossibleValue::new("rgba"),
            Self::RGB => PossibleValue::new("rgb"),
            Self::YCbCr => PossibleValue::new("ycbcr"),
            Self::GRAYSCALE => PossibleValue::new("grayscale"),
            Self::LumaA => PossibleValue::new("lumaA"),
            Self::Luma => PossibleValue::new("luma")
        })
    }
}

impl std::str::FromStr for IColorSpace {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {s}"))
    }
}
#[derive(Copy, Clone, Debug)]
pub enum IResizeMethod {
    Lanczos,
    Lanczos2,
    Bilinear,
    Bicubic,
    CatmullRom, // Catmull-Rom spline (B=0, C=0.5)
    Mitchell,   // Mitchell filter (B=1/3, C=1/3) - same as Bicubic but explicit
    BSpline,    // B-Spline (B=1, C=0)
    Hermite,    // Hermite filter (B=0, C=0)
    Sinc
}

impl IResizeMethod {
    pub const fn to_resize_method(self) -> ResizeMethod {
        match self {
            IResizeMethod::Lanczos => ResizeMethod::Lanczos3,
            IResizeMethod::Bilinear => ResizeMethod::Bilinear,
            IResizeMethod::Bicubic => ResizeMethod::Bicubic,
            IResizeMethod::Lanczos2 => ResizeMethod::Lanczos2,
            IResizeMethod::CatmullRom => ResizeMethod::CatmullRom,
            IResizeMethod::Mitchell => ResizeMethod::Mitchell,
            IResizeMethod::BSpline => ResizeMethod::BSpline,
            IResizeMethod::Hermite => ResizeMethod::Hermite,
            IResizeMethod::Sinc => ResizeMethod::Sinc
        }
    }
}

impl ValueEnum for IResizeMethod {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Lanczos,
            Self::Bilinear,
            Self::Bicubic,
            Self::Lanczos2,
            Self::CatmullRom,
            Self::Mitchell,
            Self::BSpline,
            Self::Hermite,
            Self::Sinc
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::Lanczos => PossibleValue::new("lanczos"),
            Self::Bilinear => PossibleValue::new("bilinear"),
            Self::Bicubic => PossibleValue::new("bicubic"),
            Self::Lanczos2 => PossibleValue::new("lanczos2"),
            Self::CatmullRom => PossibleValue::new("catmullrom"),
            Self::Mitchell => PossibleValue::new("mitchell"),
            Self::BSpline => PossibleValue::new("bspline"),
            Self::Hermite => PossibleValue::new("hermite"),
            Self::Sinc => PossibleValue::new("sinc")
        })
    }
}

impl std::str::FromStr for IResizeMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {s}"))
    }
}
