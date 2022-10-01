use std::fmt::{Debug, Formatter};

use zune_core::colorspace::ColorSpace;

pub enum ImgErrors
{
    JpegDecodeErrors(zune_jpeg::errors::DecodeErrors),
    NoImageForOperations,
    NoImageBuffer,
    OperationsError(ImgOperationsErrors),
    EncodeErrors(ImgEncodeErrors),
}

pub enum ImgOperationsErrors
{
    WrongColorspace(ColorSpace, ColorSpace),
    WrongComponents(usize, usize),
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
            Self::JpegDecodeErrors(ref error) =>
            {
                writeln!(f, "Jpeg decoding failed:{:?}", error)
            }
            Self::NoImageForOperations =>
            {
                writeln!(f, "No image found for which we can execute operations")
            }
            Self::NoImageBuffer => writeln!(f, "No image buffer present"),

            Self::OperationsError(ref error) => writeln!(f, "{:?}", error),

            Self::EncodeErrors(ref err) => writeln!(f, "{:?}", err),
        }
    }
}
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
