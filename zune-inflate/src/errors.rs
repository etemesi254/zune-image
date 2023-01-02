use std::fmt::{Debug, Formatter};

/// A struct returned when decompression fails
pub struct InflateDecodeErrors
{
    /// reason why decompression fails
    pub error: DecodeErrorStatus,
    /// Decoded data up until that decompression error
    pub data:  Vec<u8>
}

impl InflateDecodeErrors
{
    /// Create a new decode wrapper with data being
    /// how many bytes we actually decoded before hitting an error
    pub fn new(error: DecodeErrorStatus, data: Vec<u8>) -> InflateDecodeErrors
    {
        InflateDecodeErrors { error, data }
    }
    /// Create a new decode wrapper with an empty vector
    pub fn new_with_error(error: DecodeErrorStatus) -> InflateDecodeErrors
    {
        InflateDecodeErrors::new(error, vec![])
    }
}

impl Debug for InflateDecodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        writeln!(f, "{:?}", self.error)
    }
}

pub enum DecodeErrorStatus
{
    /// Input data is not enough to construct
    /// a full output
    InsufficientData,
    /// Anything that isn't significant
    Generic(&'static str),
    GenericStr(String),
    ///Input data was malformed.
    CorruptData,
    /// Limit set by the user was exceeded by
    /// decompressed output
    OutputLimitExceeded(usize, usize),
    /// Output CRC does not match stored CRC.
    ///
    /// Only present for zlib
    MismatchedCRC(u32, u32),
    /// Output Adler does not match stored adler
    ///
    /// Only present for gzip
    MismatchedAdler(u32, u32)
}

impl Debug for DecodeErrorStatus
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
