//! ISO Base Media File Format (ISOBMFF) box headers.
//!
//! ISOBMFF (ISO/IEC 14496-12) is the container format behind MP4 and
//! QuickTime, and behind several image formats: HEIF (HEIC and AVIF),
//! JPEG 2000 and JPEG XL's container. A file is a sequence of *boxes*; each
//! box is a header followed by a payload, and payloads often contain more
//! boxes.
//!
//! This crate parses the box header, which is the same in every format:
//! [`BoxHeader::read`] returns the box's type ([`FourCC`]), its size and
//! where it starts. What the payload means is format specific and is left to
//! the format's decoder (e.g. `zune-heic`).
//!
//! # Box header
//!
//! ```text
//!  ┌──────────┬──────────┬───────────────────────────┬────────────────────────────┐
//!  │ size (4) │ type (4) │ [largesize (8) if size=1] │ [usertype (16) if 'uuid']  │
//!  └──────────┴──────────┴───────────────────────────┴────────────────────────────┘
//! ```
//!
//! All values are big endian. `size` covers the whole box, header included:
//! - `size >= 8`: the box is `size` bytes long ([`BoxSize::Absolute`]),
//! - `size == 1`: the real size is the 64-bit `largesize` that follows
//!   ([`BoxSize::Absolute`]),
//! - `size == 0`: the box extends to the end of the file ([`BoxSize::ToEnd`]).
//!
//! The type is four bytes, almost always printable ASCII (`ftyp`, `meta`,
//! `mdat`, ...). The type `uuid` is followed by a 16-byte UUID that names a
//! vendor-defined box type ([`BoxHeader::get_uuid`]).
//!
//! The header is 8, 16, 24 or 32 bytes long ([`BoxHeader::get_header_size`]).
//!
//! # Example
//!
//! List the top-level boxes of a file:
//!
//! ```
//! use zune_core::bytestream::{ZCursor, ZReader, ZSeekFrom};
//! use zune_isobmff::{BoxHeader, BoxSize, IsoBmffErrors};
//!
//! fn list_boxes(data: &[u8]) -> Result<Vec<String>, IsoBmffErrors> {
//!     let mut reader = ZReader::new(ZCursor::new(data));
//!     let mut boxes = Vec::new();
//!     while !reader.eof()? {
//!         let header = BoxHeader::read(&mut reader)?;
//!         boxes.push(header.get_box_type().to_string());
//!         match header.get_total_size() {
//!             // jump to the next box
//!             BoxSize::Absolute(size) => {
//!                 reader.seek(ZSeekFrom::Start(header.get_initial_offset() + size))?;
//!             }
//!             // the last box
//!             BoxSize::ToEnd => break,
//!         }
//!     }
//!     Ok(boxes)
//! }
//!
//! // an 'ftyp' box (16 bytes) followed by an 'mdat' box that runs to the end
//! let data = [
//!     0, 0, 0, 16, b'f', b't', b'y', b'p', b'h', b'e', b'i', b'c', 0, 0, 0, 0,
//!     0, 0, 0, 0, b'm', b'd', b'a', b't', 1, 2, 3,
//! ];
//! assert_eq!(list_boxes(&data).unwrap(), ["ftyp", "mdat"]);
//! ```
//!
//! # Features
//!
//! The crate is `no_std` (it needs `alloc`) and has no `unsafe` code.
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg_attr(test, macro_use)]
extern crate alloc;
extern crate core;

mod box_header;
mod errors;
mod fourcc;

pub use box_header::BoxHeader;
pub use box_header::BoxSize;
pub use errors::IsoBmffErrors;
pub use fourcc::FourCC;
