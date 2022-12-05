//! PSDDecodeErrors possible during image processing
use std::fmt::{Debug, Formatter};

use zune_core::colorspace::ColorSpace;

/// All possible image errors that can occur.
///
/// This is the grandfather of image errors and contains
/// all decoding,processing and encoding errors possible
pub enum ImgErrors
{
    #[cfg(feature = "zune-jpeg")]
    JpegDecodeErrors(zune_jpeg::errors::DecodeErrors),
    #[cfg(feature = "png")]
    PngDecodeErrors(zune_png::error::PngErrors),
    #[cfg(feature = "ppm")]
    PPMDecodeErrors(zune_ppm::PPMDecodeErrors),
    #[cfg(feature = "psd")]
    PSDDecodeErrors(zune_psd::errors::PSDDecodeErrors),
    #[cfg(feature = "qoi")]
    QoiDecodeErrors(zune_qoi::QoiErrors),

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
    #[cfg(feature = "ppm")]
    PPMEncodeErrors(zune_ppm::PPMErrors)
}

impl Debug for ImgErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            #[cfg(feature = "jpeg")]
            Self::JpegDecodeErrors(ref error) =>
            {
                writeln!(f, "Jpeg decoding failed:{:?}", error)
            }
            #[cfg(feature = "png")]
            Self::PngDecodeErrors(ref error) =>
            {
                writeln!(f, "Png decoding failed:{:?}", error)
            }
            #[cfg(feature = "ppm")]
            Self::PPMDecodeErrors(ref error) =>
            {
                writeln!(f, "PPM decoding failed:{:?}", error)
            }
            #[cfg(feature = "psd")]
            Self::PSDDecodeErrors(ref error) =>
            {
                writeln!(f, "PSD decoding failed:{:?}", error)
            }
            #[cfg(feature = "qoi")]
            Self::QoiDecodeErrors(ref error) =>
            {
                writeln!(f, "QOI decoding failed:{:?}", error)
            }
            Self::GenericStr(err) =>
            {
                writeln!(f, "{}", err)
            }

            Self::GenericString(err) =>
            {
                writeln!(f, "{}", err)
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

            Self::OperationsError(ref error) => writeln!(f, "{:?}", error),

            Self::EncodeErrors(ref err) => writeln!(f, "{:?}", err),
            ImgErrors::UnsupportedColorspace(present, operation, supported) =>
            {
                writeln!(f,"Unsupported colorspace {:?}, for the operation {}\nSupported colorspaces are {:?}",present,operation,supported)
            }
            ImgErrors::DimensionsMisMatch(expected, found) =>
            {
                writeln!(
                    f,
                    "Dimensions mismatch, expected {} but found {}",
                    expected, found
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
        ImgErrors::JpegDecodeErrors(from)
    }
}

#[cfg(feature = "zune-png")]
impl From<zune_png::error::PngErrors> for ImgErrors
{
    fn from(from: zune_png::error::PngErrors) -> Self
    {
        ImgErrors::PngDecodeErrors(from)
    }
}

#[cfg(feature = "ppm")]
impl From<zune_ppm::PPMDecodeErrors> for ImgErrors
{
    fn from(from: zune_ppm::PPMDecodeErrors) -> Self
    {
        ImgErrors::PPMDecodeErrors(from)
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
                writeln!(f, "{:}", reason)
            }
            Self::Generic(reason) =>
            {
                writeln!(f, "{:}", reason)
            }
            Self::GenericString(err) =>
            {
                writeln!(f, "{}", err)
            }
            Self::WrongColorspace(ref expected, ref found) =>
            {
                writeln!(
                    f,
                    "Expected {:?} colorspace but found {:?}",
                    expected, found
                )
            }
            Self::WrongComponents(expected, found) =>
            {
                writeln!(f, "Expected {} components and found {}", expected, found)
            }
        }
    }
}

impl Debug for ImgEncodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::Generic(ref string) => writeln!(f, "{}", string),
            Self::GenericStatic(ref string) => writeln!(f, "{}", string),
            Self::UnsupportedColorspace(ref found, ref expected) =>
            {
                writeln!(
                    f,
                    "Found colorspace {:?} but the encoder supports {:?}",
                    found, expected
                )
            }
            #[cfg(feature = "ppm")]
            Self::PPMEncodeErrors(ref error) =>
            {
                writeln!(f, "{:?}", error)
            }
        }
    }
}

#[cfg(feature = "ppm")]
impl From<zune_ppm::PPMErrors> for ImgEncodeErrors
{
    fn from(error: zune_ppm::PPMErrors) -> Self
    {
        ImgEncodeErrors::PPMEncodeErrors(error)
    }
}

#[cfg(feature = "psd")]
impl From<zune_psd::errors::PSDDecodeErrors> for ImgErrors
{
    fn from(error: zune_psd::errors::PSDDecodeErrors) -> Self
    {
        ImgErrors::PSDDecodeErrors(error)
    }
}

#[cfg(feature = "qoi")]
impl From<zune_qoi::QoiErrors> for ImgErrors
{
    fn from(error: zune_qoi::QoiErrors) -> Self
    {
        ImgErrors::QoiDecodeErrors(error)
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
