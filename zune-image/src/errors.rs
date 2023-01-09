//! PSDDecodeErrors possible during image processing
use std::fmt::{Debug, Formatter};

use zune_core::colorspace::ColorSpace;

/// All possible image errors that can occur.
///
/// This is the grandfather of image errors and contains
/// all decoding,processing and encoding errors possible
pub enum ImgErrors
{
    ImageDecodeErrors(String),
    DimensionsMisMatch(usize, usize),
    UnsupportedColorspace(ColorSpace, &'static str, &'static [ColorSpace]),
    NoImageForOperations,
    NoImageForEncoding,
    NoImageBuffer,
    OperationsError(ImgOperationsErrors),
    EncodeErrors(ImgEncodeErrors),
    GenericString(String),
    GenericStr(&'static str)
}

/// PSDDecodeErrors that may occur during image operations
pub enum ImgOperationsErrors
{
    /// Unexpected colorspace
    WrongColorspace(ColorSpace, ColorSpace),
    /// Wrong number of components
    WrongComponents(usize, usize),
    /// Channel layout does not match expected
    InvalidChannelLayout(&'static str),
    /// Generic errors
    Generic(&'static str),
    /// Generic errors which have more context
    GenericString(String)
}

/// All errors possible during image encoding
pub enum ImgEncodeErrors
{
    Generic(String),
    GenericStatic(&'static str),
    UnsupportedColorspace(ColorSpace, &'static [ColorSpace]),
    ImageEncodeErrors(String)
}

impl Debug for ImgErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::ImageDecodeErrors(err) =>
            {
                writeln!(f, "{err}")
            }

            Self::GenericStr(err) =>
            {
                writeln!(f, "{err}")
            }

            Self::GenericString(err) =>
            {
                writeln!(f, "{err}")
            }
            Self::NoImageForOperations =>
            {
                writeln!(f, "No image found for which we can execute operations")
            }
            Self::NoImageForEncoding =>
            {
                writeln!(f, "No image found for which we can encode")
            }
            Self::NoImageBuffer => writeln!(f, "No image buffer present"),

            Self::OperationsError(ref error) => writeln!(f, "{error:?}"),

            Self::EncodeErrors(ref err) => writeln!(f, "{err:?}"),
            ImgErrors::UnsupportedColorspace(present, operation, supported) =>
            {
                writeln!(f, "Unsupported colorspace {present:?}, for the operation {operation}\nSupported colorspaces are {supported:?}")
            }
            ImgErrors::DimensionsMisMatch(expected, found) =>
            {
                writeln!(
                    f,
                    "Dimensions mismatch, expected {expected} but found {found}"
                )
            }
        }
    }
}
#[cfg(feature = "zune-jpeg")]
impl From<zune_jpeg::errors::DecodeErrors> for ImgErrors
{
    fn from(from: zune_jpeg::errors::DecodeErrors) -> Self
    {
        let err = format!("jpg: {from:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}

#[cfg(feature = "zune-png")]
impl From<zune_png::error::PngErrors> for ImgErrors
{
    fn from(from: zune_png::error::PngErrors) -> Self
    {
        let err = format!("png: {from:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}

#[cfg(feature = "ppm")]
impl From<zune_ppm::PPMDecodeErrors> for ImgErrors
{
    fn from(from: zune_ppm::PPMDecodeErrors) -> Self
    {
        let err = format!("ppm: {from:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}

impl From<ImgOperationsErrors> for ImgErrors
{
    fn from(from: ImgOperationsErrors) -> Self
    {
        ImgErrors::OperationsError(from)
    }
}

impl From<ImgEncodeErrors> for ImgErrors
{
    fn from(from: ImgEncodeErrors) -> Self
    {
        ImgErrors::EncodeErrors(from)
    }
}
impl Debug for ImgOperationsErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::InvalidChannelLayout(reason) =>
            {
                writeln!(f, "{reason:}")
            }
            Self::Generic(reason) =>
            {
                writeln!(f, "{reason:}")
            }
            Self::GenericString(err) =>
            {
                writeln!(f, "{err}")
            }
            Self::WrongColorspace(ref expected, ref found) =>
            {
                writeln!(f, "Expected {expected:?} colorspace but found {found:?}")
            }
            Self::WrongComponents(expected, found) =>
            {
                writeln!(f, "Expected {expected} components and found {found}")
            }
        }
    }
}

#[cfg(feature = "ppm")]
impl From<zune_ppm::PPMErrors> for ImgEncodeErrors
{
    fn from(error: zune_ppm::PPMErrors) -> Self
    {
        let err = format!("ppm: {error:?}");

        ImgEncodeErrors::ImageEncodeErrors(err)
    }
}

#[cfg(feature = "psd")]
impl From<zune_psd::errors::PSDDecodeErrors> for ImgErrors
{
    fn from(error: zune_psd::errors::PSDDecodeErrors) -> Self
    {
        let err = format!("psd: {error:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}

#[cfg(feature = "qoi")]
impl From<zune_qoi::QoiErrors> for ImgErrors
{
    fn from(error: zune_qoi::QoiErrors) -> Self
    {
        let err = format!("qoi: {error:?}");

        ImgErrors::ImageDecodeErrors(err)
    }
}

impl From<String> for ImgErrors
{
    fn from(s: String) -> ImgErrors
    {
        ImgErrors::GenericString(s)
    }
}

impl From<&'static str> for ImgErrors
{
    fn from(s: &'static str) -> ImgErrors
    {
        ImgErrors::GenericStr(s)
    }
}

impl Debug for ImgEncodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::Generic(ref string) => writeln!(f, "{string}"),
            Self::GenericStatic(ref string) => writeln!(f, "{string}"),
            Self::UnsupportedColorspace(ref found, ref expected) =>
            {
                writeln!(
                    f,
                    "Found colorspace {found:?} but the encoder supports {expected:?}"
                )
            }
            Self::ImageEncodeErrors(err) =>
            {
                writeln!(f, "Image could not be encoded, reason: {err}")
            }
        }
    }
}
