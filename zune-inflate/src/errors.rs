use std::fmt::{Debug, Formatter};

pub enum ZlibDecodeErrors
{
    InsufficientData,
    Generic(&'static str),
    GenericStr(String),
    CorruptData
}

impl Debug for ZlibDecodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::InsufficientData => writeln!(f, "Insufficient data"),
            Self::Generic(reason) => writeln!(f, "{}", reason),
            Self::GenericStr(reason) => writeln!(f, "{}", reason),
            Self::CorruptData => writeln!(f, "Corrupt data")
        }
    }
}
