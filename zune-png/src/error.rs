use std::fmt::{Debug, Formatter};

pub enum PngErrors
{
    BadSignature,
    GenericStatic(&'static str),
    Generic(String),
    BadCrc(u32, u32),
    ZlibDecodeErrors(zune_inflate::errors::InflateDecodeErrors)
}
impl Debug for PngErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::BadSignature => writeln!(f, "Bad PNG signature, not a png"),
            Self::GenericStatic(val) => writeln!(f, "{val:?}"),
            Self::Generic(val) => writeln!(f, "{val:?}"),
            Self::BadCrc(expected, found) => writeln!(
                f,
                "CRC does not match, expected {expected} but found {found}",
            ),
            Self::ZlibDecodeErrors(err) =>
            {
                writeln!(f, "Error decoding idat chunks {err:?}")
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
