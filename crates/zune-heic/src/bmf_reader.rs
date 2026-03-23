//!
//! Parses the 8-byte (or 16-byte for extended size) ISOBMFF box header.
//!
//!  ┌────────────┬──────────┬───────────────────────────┐
//!  │ size32 (4) │ type (4) │ [largesize (8) if size=1] │
//!  └────────────┴──────────┴───────────────────────────┘
//!
//! Additionally handles the 'uuid' extended box type.

use core::fmt;

use zune_core::bytestream::{ZByteReaderTrait, ZReader};

use crate::errors::HeicErrors;

/// Four-character code identifying the box type.
#[derive(Clone, PartialEq, Eq, Hash, Copy)]
pub struct FourCC(pub [u8; 4]);

impl FourCC {
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or("????")
    }
    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
}

impl fmt::Display for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Debug for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FourCC(\"{}\")", self.as_str())
    }
}

impl From<&[u8; 4]> for FourCC {
    fn from(b: &[u8; 4]) -> Self {
        FourCC(*b)
    }
}

impl PartialEq<&str> for FourCC {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone)]
pub enum BoxSize {
    /// Absolute item length
    Absolute(u64),
    /// header extends to the end
    ToEnd
}
/// Parsed header of a single ISOBMFF box.
#[derive(Debug, Clone)]
pub struct BoxHeader {
    /// Absolute byte offset where this box starts.
    pub _offset:     u64,
    /// Total size of the box in bytes (header + payload).
    pub total_size:  BoxSize,
    /// The four-character box type.
    pub box_type:    FourCC,
    /// For 'uuid' boxes: the 16-byte UUID following the type field.
    pub _uuid:       Option<[u8; 16]>,
    /// Byte length of the header itself (8 or 16 for extended size, +16 for uuid).
    pub header_size: u64
}

impl BoxHeader {
    /// Try to parse a BoxHeader from the current reader position.
    pub fn read<T: ZByteReaderTrait>(reader: &mut ZReader<T>) -> Result<Self, HeicErrors> {
        let offset = reader.position()?;

        let size32 = reader.get_u32_be_err()?;
        let box_type = reader.read_fixed_bytes_or_error::<4>()?;

        let mut header_size: u64 = 8;

        // Resolve actual total size.
        let total_size = match size32 {
            0 => {
                // Box extends to the end of the container.
                BoxSize::ToEnd
            }
            1 => {
                header_size = 16;
                BoxSize::Absolute(reader.get_u64_be_err()?)
            }
            s => BoxSize::Absolute(s as u64)
        };

        // Handle uuid extended type.
        let uuid = if &box_type == b"uuid" {
            let mut u = [0u8; 16];
            reader.read_exact_bytes(&mut u)?;
            header_size += 16;
            Some(u)
        } else {
            None
        };

        Ok(BoxHeader {
            _offset: offset,
            total_size,
            box_type: FourCC(box_type),
            _uuid: uuid,
            header_size
        })
    }
}
