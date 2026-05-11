use std::fmt::{Debug, Formatter};

pub enum PngErrors
{
    BadSignature,
    GenericStatic(&'static str),
    Generic(String),
    BadCrc(u32, u32),
    ZlibDecodeErrors(zune_inflate::errors::InflateDecodeErrors),
    EmptyPalette
}
impl Debug for PngErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::BadSignature => write!(f, "Bad PNG signature, not a png"),
            Self::GenericStatic(val) => write!(f, "{val:?}"),
            Self::Generic(val) => write!(f, "{val:?}"),
            Self::BadCrc(expected, found) => write!(
                f,
                "CRC does not match, expected {expected} but found {found}",
            ),
            Self::ZlibDecodeErrors(err) =>
            {
                write!(f, "Error decoding idat chunks {err:?}")
            }
            Self::EmptyPalette =>
            {
                write!(f, "Empty palette but image is indexed")
            }
        }
    }
}
impl From<&'static str> for PngErrors
{
    fn from(val: &'static str) -> Self
    {
        Self::GenericStatic(val)
    }
}

impl From<String> for PngErrors
{
    fn from(val: String) -> Self
    {
        Self::Generic(val)
    }
}

impl From<zune_inflate::errors::InflateDecodeErrors> for PngErrors
{
    fn from(val: zune_inflate::errors::InflateDecodeErrors) -> Self
    {
        Self::ZlibDecodeErrors(val)
    }
}
