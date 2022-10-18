use std::fmt::{Debug, Formatter};

use zune_core::colorspace::ColorSpace;

pub enum ImgErrors
{
    #[cfg(feature = "zune-jpeg")]
    JpegDecodeErrors(zune_jpeg::errors::DecodeErrors),
    NoImageForOperations,
    NoImageForEncoding,
    NoImageBuffer,
    OperationsError(ImgOperationsErrors),
    EncodeErrors(ImgEncodeErrors),
    GenericString(String),
    GenericStr(&'static str),
}

pub enum ImgOperationsErrors
{
    WrongColorspace(ColorSpace, ColorSpace),
    WrongComponents(usize, usize),
    InvalidChannelLayout(&'static str),
    Generic(&'static str),
    GenericString(String),
}

pub enum ImgEncodeErrors
{
    Generic(String),
    GenericStatic(&'static str),
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
        }
    }
}

impl From<String> for ImgErrors
{
    fn from(s: String) -> ImgErrors
    {
        ImgErrors::GenericString(s)
    }
}
