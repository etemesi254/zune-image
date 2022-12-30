use std::fmt::{Debug, Formatter};

pub enum QoiErrors
{
    WrongMagicBytes,
    InsufficientData(usize, usize),
    UnknownChannels(u8),
    UnknownColorspace(u8),
    Generic(String),
    GenericStatic(&'static str)
}

impl Debug for QoiErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            QoiErrors::WrongMagicBytes =>
            {
                writeln!(f, "Wrong magic bytes, expected `qoif` as image start")
            }
            QoiErrors::InsufficientData(expected, found) =>
            {
                writeln!(
                    f,
                    "Insufficient data required {expected} but remaining stream has {found}"
                )
            }
            QoiErrors::UnknownChannels(channel) =>
            {
                writeln!(
                    f,
                    "Unknown channel number {channel}, expected either 3 or 4"
                )
            }
            QoiErrors::UnknownColorspace(colorspace) =>
            {
                writeln!(
                    f,
                    "Unknown colorspace number {colorspace}, expected either 0 or 1"
                )
            }
            QoiErrors::Generic(val) =>
            {
                writeln!(f, "{val}")
            }
            QoiErrors::GenericStatic(val) =>
            {
                writeln!(f, "{val}")
            }
        }
    }
}

impl From<&'static str> for QoiErrors
{
    fn from(r: &'static str) -> Self
    {
        Self::GenericStatic(r)
    }
}
