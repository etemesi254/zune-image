use std::fmt::{Debug, Formatter};

pub struct DecodeWrapper
{
    pub error: DecodeErrors,
    pub data:  Vec<u8>
}

impl Debug for DecodeWrapper
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        writeln!(f, "{:?}", self.error)
    }
}

pub enum DecodeErrors
{
    InsufficientData,
    Generic(&'static str),
    GenericStr(String),
    CorruptData,
    OutputLimitExceeded(usize, usize),
    MismatchedCRC(u32, u32),
    MismatchedAdler(u32, u32)
}

impl Debug for DecodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::InsufficientData => writeln!(f, "Insufficient data"),
            Self::Generic(reason) => writeln!(f, "{reason}"),
            Self::GenericStr(reason) => writeln!(f, "{reason}"),
            Self::CorruptData => writeln!(f, "Corrupt data"),
            Self::OutputLimitExceeded(limit, current) => writeln!(
                f,
                "Output limit exceeded, set limit was {limit} and output size is {current}"
            ),
            Self::MismatchedCRC(expected, found) =>
            {
                writeln!(f, "Mismatched CRC, expected {expected} but found {found}")
            }
            Self::MismatchedAdler(expected, found) =>
            {
                writeln!(f, "Mismatched Adler, expected {expected} but found {found}")
            }
        }
    }
}
