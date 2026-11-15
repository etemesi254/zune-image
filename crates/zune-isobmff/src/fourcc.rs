use core::fmt;

use zune_core::bytestream::{ZByteIoError, ZByteWriterTrait, ZWriter};

/// Four-character code (4CC) identifying a box type, such as `ftyp` or
/// `meta`.
///
/// The bytes are kept as they are in the file; they are usually, but not
/// necessarily, printable ASCII.
///
/// ```
/// use zune_isobmff::FourCC;
///
/// let fourcc = FourCC(*b"ftyp");
/// assert_eq!(fourcc, "ftyp");
/// assert_eq!(fourcc, b"ftyp");
/// assert_eq!(fourcc.to_string(), "ftyp");
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Copy)]
pub struct FourCC(pub [u8; 4]);

impl FourCC {
    /// The FourCC as a string.
    ///
    /// Returns `"????"` if the bytes are not valid UTF-8.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.0).unwrap_or("????")
    }

    /// The four bytes of the FourCC.
    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    /// Write the four bytes to `writer`.
    ///
    /// # Errors
    /// If the writer cannot take four more bytes.
    pub fn encode<T: ZByteWriterTrait>(&self, writer: &mut ZWriter<T>) -> Result<(), ZByteIoError> {
        writer.write_all(self.as_bytes())
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

impl PartialEq<&[u8; 4]> for FourCC {
    fn eq(&self, other: &&[u8; 4]) -> bool {
        self.as_bytes() == *other
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    #[test]
    fn test_construct() {
        let uut = crate::FourCC([b'a', b'b', b'c', b'd']);
        assert_eq!(uut.as_str(), "abcd");
    }

    #[test]
    fn test_display() {
        let uut = crate::FourCC([0x61, 0x62, 0x63, 0x64]);
        assert_eq!(format!("{uut}"), "abcd");
    }

    #[test]
    fn test_debug() {
        let uut = crate::FourCC([0x61, 0x62, 0x63, 0x64]);
        assert_eq!(format!("{uut:?}"), "FourCC(\"abcd\")");
    }

    #[test]
    fn test_construct_from() {
        let uut = crate::FourCC::from(&[0x61u8, 0x62u8, 0x63u8, 0x64u8]);
        assert_eq!(uut.as_str(), "abcd");
    }

    #[test]
    fn test_partial_eq_bytes() {
        let uut = crate::FourCC([b'a', b'b', b'c', b'd']);
        assert_eq!(uut, &[0x61u8, 0x62u8, 0x63u8, 0x64u8]);
    }

    #[test]
    fn test_partial_eq_str() {
        let uut = crate::FourCC([b'a', b'b', b'c', b'd']);
        assert_eq!(uut, "abcd");
    }

    #[allow(dead_code)] // used in this test module
    fn test_write_fourcc(fourcc: crate::FourCC, expected_bytes: Vec<u8>) {
        let mut output = vec![];
        let mut sink = zune_core::bytestream::ZWriter::new(&mut output);
        fourcc.encode(&mut sink).unwrap();
        assert_eq!(sink.bytes_written(), 4);
        assert_eq!(output, expected_bytes);
    }

    #[test]
    fn test_write_abcd() {
        test_write_fourcc(
            crate::FourCC([b'a', b'b', b'c', b'd']),
            vec![0x61u8, 0x62u8, 0x63u8, 0x64u8],
        );
    }

    #[test]
    fn test_write_xml() {
        test_write_fourcc(
            crate::FourCC([b'x', b'm', b'l', b' ']),
            vec![0x78u8, 0x6du8, 0x6cu8, 0x20u8],
        );
    }
}
