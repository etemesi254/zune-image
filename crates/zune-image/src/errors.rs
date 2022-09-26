use std::fmt::{Debug, Formatter};

pub enum ImgDecodeErrors
{
    JpegDecodeErrors(zune_jpeg::errors::DecodeErrors),
}

impl Debug for ImgDecodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::JpegDecodeErrors(ref error) =>
            {
                writeln!(f, "Jpeg decoding failed:{:?}", error)
            }
        }
    }
}
impl From<zune_jpeg::errors::DecodeErrors> for ImgDecodeErrors
{
    fn from(from: zune_jpeg::errors::DecodeErrors) -> Self
    {
        ImgDecodeErrors::JpegDecodeErrors(from)
    }
}
