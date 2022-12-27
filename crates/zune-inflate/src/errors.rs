use std::fmt::{Debug, Formatter};

pub enum DecodeErrors
{
    InsufficientData,
    Generic(&'static str),
    GenericStr(String),
    CorruptData,
    OutputLimitExceeded(usize, usize)
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
            )
        }
    }
}
