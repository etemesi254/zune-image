//! Box header parsing.
//!
//! Parses the 8-byte (or 16-byte for extended size) ISOBMFF box header.
//!
//!  ┌────────────┬──────────┬───────────────────────────┐
//!  │ size32 (4) │ type (4) │ [largesize (8) if size=1] │
//!  └────────────┴──────────┴───────────────────────────┘
//!
//! Additionally handles the 'uuid' extended box type.

use zune_core::bytestream::{ZByteReaderTrait, ZReader};

use crate::FourCC;
use crate::IsoBmffErrors;

/// Size of a box, from its header.
#[derive(Debug, Clone)]
pub enum BoxSize {
    /// The box is this many bytes long, header included.
    Absolute(u64),
    /// The box extends to the end of the file (a `size` of 0).
    ///
    /// Only the last box of a file can have this size.
    ToEnd,
}

/// Parsed header of a single ISOBMFF box.
///
/// Read with [`BoxHeader::read`]. After reading, the reader is positioned at
/// the start of the box's payload, which is
/// [`get_header_size`](Self::get_header_size) bytes after
/// [`get_initial_offset`](Self::get_initial_offset).
///
/// ```
/// use zune_core::bytestream::{ZCursor, ZReader};
/// use zune_isobmff::{BoxHeader, BoxSize};
///
/// let data = [0, 0, 0, 12, b'f', b'r', b'e', b'e', 1, 2, 3, 4];
/// let mut reader = ZReader::new(ZCursor::new(&data[..]));
/// let header = BoxHeader::read(&mut reader).unwrap();
///
/// assert_eq!(header.get_box_type(), &"free");
/// assert_eq!(header.get_header_size(), 8);
/// assert!(matches!(header.get_total_size(), BoxSize::Absolute(12)));
/// // the reader is now at the payload
/// assert_eq!(reader.position().unwrap(), 8);
/// ```
#[derive(Debug, Clone)]
pub struct BoxHeader {
    /// Absolute byte offset where this box starts.
    offset: u64,
    /// Total size of the box in bytes (header + payload).
    total_size: BoxSize,
    /// The four-character box type.
    box_type: FourCC,
    /// For 'uuid' boxes: the 16-byte UUID following the type field.
    uuid: Option<[u8; 16]>,
    /// Byte length of the header itself (8 or 16 for extended size, +16 for uuid).
    header_size: u64,
}

impl BoxHeader {
    /// Parse a box header at the reader's current position.
    ///
    /// On success the reader is left at the start of the box's payload. The
    /// size is not checked against the data available, nor against the
    /// header size: callers should not trust it.
    ///
    /// # Errors
    /// [`IsoBmffErrors::Io`] if the data ends before the header does.
    pub fn read<T: ZByteReaderTrait>(reader: &mut ZReader<T>) -> Result<Self, IsoBmffErrors> {
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
            s => BoxSize::Absolute(u64::from(s)),
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
            offset,
            total_size,
            box_type: FourCC(box_type),
            uuid,
            header_size,
        })
    }

    /// Initial offset for this box.
    ///
    /// This is the start of the header (the first byte of the  `size` value),
    /// and not the start of the content.
    pub fn get_initial_offset(&self) -> u64 {
        self.offset
    }

    /// Size of the header for this box: 8 bytes, plus 8 for a 64-bit size
    /// and 16 for a `uuid` box's UUID.
    ///
    /// The payload starts this many bytes after
    /// [`get_initial_offset`](Self::get_initial_offset).
    pub fn get_header_size(&self) -> u64 {
        self.header_size
    }

    /// Total size for this box.
    ///
    /// This includes the header and payload bytes.
    pub fn get_total_size(&self) -> &BoxSize {
        &self.total_size
    }

    /// Box type identifying the box structure.
    pub fn get_box_type(&self) -> &FourCC {
        &self.box_type
    }

    /// The UUID defining the specific box, if using the `uuid` box type.
    ///
    /// This will be None for any other box type.
    pub fn get_uuid(&self) -> Option<[u8; 16]> {
        self.uuid
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn check_standard_header() {
        let data = vec![
            0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x63, 0x00, 0x00,
            0x00, 0x00, 0x6d, 0x69, 0x66, 0x31, 0x68, 0x65, 0x69, 0x63,
        ];
        let mut stream = zune_core::bytestream::ZReader::new(zune_core::bytestream::ZCursor::new(
            data.as_slice(),
        ));
        let box_header = super::BoxHeader::read(&mut stream).expect("parsed OK");
        assert_eq!(box_header.get_initial_offset(), 0);
        assert_eq!(box_header.header_size, 8);
        assert_eq!(box_header.box_type, crate::FourCC(*b"ftyp"));
        match box_header.get_total_size() {
            super::BoxSize::Absolute(size) => assert_eq!(*size, 24),
            super::BoxSize::ToEnd => panic!("Expected box size to be absolute"),
        }
        assert!(box_header.get_uuid().is_none());
    }

    #[test]
    fn check_largesize_header() {
        let data = vec![
            0x00, 0x00, 0x00, 0x01, 0x66, 0x74, 0x79, 0x70, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x20, 0x68, 0x65, 0x69, 0x63, 0x00, 0x00, 0x00, 0x00, 0x6d, 0x69, 0x66, 0x31,
            0x68, 0x65, 0x69, 0x63,
        ];
        let mut stream = zune_core::bytestream::ZReader::new(zune_core::bytestream::ZCursor::new(
            data.as_slice(),
        ));
        let box_header = super::BoxHeader::read(&mut stream).expect("parsed OK");
        assert_eq!(box_header.get_initial_offset(), 0);
        assert_eq!(box_header.header_size, 16);
        assert_eq!(box_header.box_type, crate::FourCC(*b"ftyp"));
        match box_header.get_total_size() {
            super::BoxSize::Absolute(size) => assert_eq!(*size, 32u64),
            super::BoxSize::ToEnd => panic!("Expected box size to be absolute"),
        }
        assert!(box_header.get_uuid().is_none());
    }

    #[test]
    fn check_to_end_header() {
        let data = vec![
            0x00, 0x00, 0x00, 0x00, b'm', b'd', b'a', b't', 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
        ];
        let mut stream = zune_core::bytestream::ZReader::new(zune_core::bytestream::ZCursor::new(
            data.as_slice(),
        ));
        let box_header = super::BoxHeader::read(&mut stream).expect("parsed OK");
        assert_eq!(box_header.get_initial_offset(), 0);
        assert_eq!(box_header.header_size, 8);
        assert_eq!(box_header.box_type, crate::FourCC(*b"mdat"));
        match box_header.get_total_size() {
            super::BoxSize::Absolute(_) => panic!("Expected box size to continue to end"),
            super::BoxSize::ToEnd => {}
        }
        assert!(box_header.get_uuid().is_none());
    }

    #[test]
    fn check_uuid_header() {
        let data = vec![
            0x00, 0x00, 0x00, 0x19, b'u', b'u', b'i', b'd', b'a', b'b', b'c', b'd', 0x00, 0x11,
            0x00, 0x10, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71, 0xff,
        ];
        let mut stream = zune_core::bytestream::ZReader::new(zune_core::bytestream::ZCursor::new(
            data.as_slice(),
        ));
        let box_header = super::BoxHeader::read(&mut stream).expect("parsed OK");
        assert_eq!(box_header.get_initial_offset(), 0);
        assert_eq!(box_header.header_size, 24);
        assert_eq!(box_header.box_type, crate::FourCC(*b"uuid"));
        match box_header.get_total_size() {
            super::BoxSize::Absolute(size) => assert_eq!(*size, 25u64),
            super::BoxSize::ToEnd => panic!("Expected box size to be absolute"),
        }
        // See ISO/IEC 14496-12:2026 Section 4.2.3 for this extensibility mechanism
        assert_eq!(
            box_header.get_uuid().unwrap(),
            [
                0x61, 0x62, 0x63, 0x64, 0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38,
                0x9b, 0x71,
            ]
        );
    }
}
