//! Errors possible during image processing
use std::any::TypeId;
use std::fmt::{Debug, Formatter};
use std::io::Error;

use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;

use crate::codecs::ImageFormat;

/// All possible image errors that can occur.
///
/// This is the grandfather of image errors and contains
/// all decoding,processing and encoding errors possible
pub enum ImageErrors
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
    GenericStr(&'static str),
    WrongTypeId(TypeId, TypeId),
    IoError(std::io::Error)
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
    /// Unsupported bit depth for an operation
    ///
    /// The current operation does not support the bit depth
    UnsupportedType(&'static str, BitType),
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
    ImageEncodeErrors(String),
    NoEncoderForFormat(ImageFormat)
}

impl Debug for ImageErrors
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
            ImageErrors::UnsupportedColorspace(present, operation, supported) =>
            {
                writeln!(f, "Unsupported colorspace {present:?}, for the operation {operation}\nSupported colorspaces are {supported:?}")
            }
            ImageErrors::DimensionsMisMatch(expected, found) =>
            {
                writeln!(
                    f,
                    "Dimensions mismatch, expected {expected} but found {found}"
                )
            }
            ImageErrors::WrongTypeId(expected, found) =>
            {
                writeln!(
                    f,
                    "Expected type with ID of {expected:?} but found {found:?}"
                )
            }
            ImageErrors::IoError(reason) =>
            {
                writeln!(f, "IO error, {:?}", reason)
            }
        }
    }
}

impl From<std::io::Error> for ImageErrors
{
    fn from(value: Error) -> Self
    {
        Self::IoError(value)
    }
}

impl From<ImgOperationsErrors> for ImageErrors
{
    fn from(from: ImgOperationsErrors) -> Self
    {
        ImageErrors::OperationsError(from)
    }
}

impl From<ImgEncodeErrors> for ImageErrors
{
    fn from(from: ImgEncodeErrors) -> Self
    {
        ImageErrors::EncodeErrors(from)
    }
}

impl Debug for ImgOperationsErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::UnsupportedType(operation, depth) =>
            {
                writeln!(
                    f,
                    "Unsupported bit type {depth:?} for operation {operation}"
                )
            }
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

impl From<String> for ImageErrors
{
    fn from(s: String) -> ImageErrors
    {
        ImageErrors::GenericString(s)
    }
}

impl From<&'static str> for ImageErrors
{
    fn from(s: &'static str) -> ImageErrors
    {
        ImageErrors::GenericStr(s)
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
            Self::NoEncoderForFormat(format) =>
            {
                writeln!(f, "No encoder for image format {:?}", format)
            }
        }
    }
}
