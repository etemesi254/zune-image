#![allow(dead_code, unused_variables)]
// sample_decoder — HEIF container decoder
//
// All code is in one file, organised as inline modules that mirror the
// original two-crate workspace layout:
//
//   mod isobmff          ← generic ISO Base Media File Format parser
//     mod error          ← Error / Result types (no external deps)
//     mod reader         ← big-endian typed reads over Read+Seek
//     mod header         ← BoxHeader, BoxType
//     mod fullbox        ← FullBox version+flags prefix
//     mod tree           ← lazy RawBox tree, parse_file()
//     mod visitor        ← BoxVisitor trait
//     mod boxes          ← generic box decoders (ftyp, hdlr, pitm, infe, iloc, ipma)
//
//   mod heif             ← HEIF-specific layer built on isobmff
//     mod boxes          ← HEIF box decoders (ispe, irot, imir, colr, auxC, hvcC, av1C, iref)
//     mod properties     ← ItemProperties (resolved per-item property bag)
//     mod file           ← HeifFile, ImageItem, ItemKind, HeifFile::parse()
//
//   fn main()            ← CLI entry point
//
// To build:
//   rustc sample_decoder -o heif_decode        (single file, no Cargo needed)
// or with Cargo (single-file project):
//   cargo init && cp sample_decoder src/sample_decoder && cargo run -- file.heic

// ══════════════════════════════════════════════════════════════════════════════
// mod isobmff
// ══════════════════════════════════════════════════════════════════════════════
use std::io::{Read, Write};
mod isobmff {

    // ── error ─────────────────────────────────────────────────────────────────
    pub mod error {
        use std::fmt;

        pub enum Error {
            /// Wraps an underlying I/O failure.
            Io(std::io::Error),
            /// A box header reported a size inconsistent with its position.
            InvalidBoxSize { offset: u64, size: u64 },
            /// The parser needed more bytes than were available.
            UnexpectedEof { offset: u64, needed: u64 },
            /// The four-byte box-type field contained non-printable bytes.
            InvalidBoxType([u8; 4]),
            /// A FullBox carried a `version` value the parser does not handle.
            UnsupportedVersion { offset: u64, version: u8 },
            /// A box payload was shorter than the minimum required by its spec.
            PayloadTooShort {
                box_type: String,
                needed:   usize,
                have:     usize
            },
            /// Any other structural problem found while parsing a specific box.
            ParseError { box_type: String, msg: String }
        }

        impl fmt::Display for Error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Error::Io(e) => write!(f, "I/O error: {}", e),
                    Error::InvalidBoxSize { offset, size } => {
                        write!(f, "Box at offset {} has invalid size {}", offset, size)
                    }
                    Error::UnexpectedEof { offset, needed } => write!(
                        f,
                        "Unexpected end of data: need {} bytes at offset {}",
                        needed, offset
                    ),
                    Error::InvalidBoxType(bytes) => {
                        write!(f, "Box type contains non-ASCII bytes: {:?}", bytes)
                    }
                    Error::UnsupportedVersion { offset, version } => write!(
                        f,
                        "FullBox at offset {} has unsupported version {}",
                        offset, version
                    ),
                    Error::PayloadTooShort {
                        box_type,
                        needed,
                        have
                    } => write!(
                        f,
                        "Payload too short for box '{}': need {}, have {}",
                        box_type, needed, have
                    ),
                    Error::ParseError { box_type, msg } => {
                        write!(f, "Parse error in box '{}': {}", box_type, msg)
                    }
                }
            }
        }

        impl fmt::Debug for Error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Error::Io(e) => f.debug_tuple("Io").field(e).finish(),
                    Error::InvalidBoxSize { offset, size } => f
                        .debug_struct("InvalidBoxSize")
                        .field("offset", offset)
                        .field("size", size)
                        .finish(),
                    Error::UnexpectedEof { offset, needed } => f
                        .debug_struct("UnexpectedEof")
                        .field("offset", offset)
                        .field("needed", needed)
                        .finish(),
                    Error::InvalidBoxType(bytes) => {
                        f.debug_tuple("InvalidBoxType").field(bytes).finish()
                    }
                    Error::UnsupportedVersion { offset, version } => f
                        .debug_struct("UnsupportedVersion")
                        .field("offset", offset)
                        .field("version", version)
                        .finish(),
                    Error::PayloadTooShort {
                        box_type,
                        needed,
                        have
                    } => f
                        .debug_struct("PayloadTooShort")
                        .field("box_type", box_type)
                        .field("needed", needed)
                        .field("have", have)
                        .finish(),
                    Error::ParseError { box_type, msg } => f
                        .debug_struct("ParseError")
                        .field("box_type", box_type)
                        .field("msg", msg)
                        .finish()
                }
            }
        }

        impl std::error::Error for Error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    Error::Io(e) => Some(e),
                    _ => None
                }
            }
        }

        impl From<std::io::Error> for Error {
            fn from(e: std::io::Error) -> Self {
                Error::Io(e)
            }
        }

        pub type Result<T> = std::result::Result<T, Error>;
    }

    // ── reader ────────────────────────────────────────────────────────────────
    pub mod reader {
        use std::io::{Read, Seek, SeekFrom};

        use super::error::Result;

        /// Typed big-endian reads over any `Read + Seek` source.
        pub struct IsobmffReader<R: Read + Seek> {
            inner:         R,
            pub total_len: u64
        }

        impl<R: Read + Seek> IsobmffReader<R> {
            pub fn new(mut inner: R) -> Result<Self> {
                let total_len = inner.seek(SeekFrom::End(0))?;
                inner.seek(SeekFrom::Start(0))?;
                Ok(Self { inner, total_len })
            }

            pub fn seek_to(&mut self, offset: u64) -> Result<()> {
                self.inner.seek(SeekFrom::Start(offset))?;
                Ok(())
            }

            pub fn pos(&mut self) -> Result<u64> {
                Ok(self.inner.seek(SeekFrom::Current(0))?)
            }

            pub fn read_u8(&mut self) -> Result<u8> {
                let mut b = [0u8; 1];
                self.inner.read_exact(&mut b)?;
                Ok(b[0])
            }

            pub fn read_u16_be(&mut self) -> Result<u16> {
                let mut b = [0u8; 2];
                self.inner.read_exact(&mut b)?;
                Ok(u16::from_be_bytes(b))
            }

            pub fn read_u32_be(&mut self) -> Result<u32> {
                let mut b = [0u8; 4];
                self.inner.read_exact(&mut b)?;
                Ok(u32::from_be_bytes(b))
            }

            pub fn read_u64_be(&mut self) -> Result<u64> {
                let mut b = [0u8; 8];
                self.inner.read_exact(&mut b)?;
                Ok(u64::from_be_bytes(b))
            }

            pub fn read_i16_be(&mut self) -> Result<i16> {
                let mut b = [0u8; 2];
                self.inner.read_exact(&mut b)?;
                Ok(i16::from_be_bytes(b))
            }

            pub fn read_i32_be(&mut self) -> Result<i32> {
                let mut b = [0u8; 4];
                self.inner.read_exact(&mut b)?;
                Ok(i32::from_be_bytes(b))
            }

            pub fn read_4cc(&mut self) -> Result<[u8; 4]> {
                let mut b = [0u8; 4];
                self.inner.read_exact(&mut b)?;
                Ok(b)
            }

            pub fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>> {
                let mut buf = vec![0u8; n];
                self.inner.read_exact(&mut buf)?;
                Ok(buf)
            }

            /// Read a null-terminated string, stopping at `\0` or `max_len` bytes.
            pub fn read_null_terminated_string(&mut self, max_len: usize) -> Result<String> {
                let mut bytes = Vec::new();
                for _ in 0..max_len {
                    let b = self.read_u8()?;
                    if b == 0 {
                        break;
                    }
                    bytes.push(b);
                }
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }

            pub fn read_fixed_point_16_16(&mut self) -> Result<f64> {
                Ok(self.read_u32_be()? as f64 / 65536.0)
            }

            pub fn read_fixed_point_8_8(&mut self) -> Result<f64> {
                Ok(self.read_u16_be()? as f64 / 256.0)
            }

            pub fn inner_mut(&mut self) -> &mut R {
                &mut self.inner
            }
        }

        // ── slice helpers ──────────────────────────────────────────────────────

        pub fn u8_at(buf: &[u8], off: usize) -> u8 {
            buf[off]
        }

        pub fn u16_be_at(buf: &[u8], off: usize) -> u16 {
            u16::from_be_bytes([buf[off], buf[off + 1]])
        }

        pub fn u32_be_at(buf: &[u8], off: usize) -> u32 {
            u32::from_be_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
        }

        pub fn u64_be_at(buf: &[u8], off: usize) -> u64 {
            u64::from_be_bytes([
                buf[off],
                buf[off + 1],
                buf[off + 2],
                buf[off + 3],
                buf[off + 4],
                buf[off + 5],
                buf[off + 6],
                buf[off + 7]
            ])
        }

        pub fn fourcc_str(b: &[u8; 4]) -> String {
            String::from_utf8_lossy(b).into_owned()
        }
    }

    // ── header ────────────────────────────────────────────────────────────────
    pub mod header {
        use std::fmt;
        use std::io::{Read, Seek};

        use super::error::{Error, Result};
        use super::reader::IsobmffReader;

        /// Four-character code identifying the box type.
        #[derive(Clone, PartialEq, Eq, Hash)]
        pub struct BoxType(pub [u8; 4]);

        impl BoxType {
            pub fn as_str(&self) -> &str {
                std::str::from_utf8(&self.0).unwrap_or("????")
            }
            pub fn as_bytes(&self) -> &[u8; 4] {
                &self.0
            }
        }

        impl fmt::Display for BoxType {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl fmt::Debug for BoxType {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "BoxType(\"{}\")", self.as_str())
            }
        }

        impl From<&[u8; 4]> for BoxType {
            fn from(b: &[u8; 4]) -> Self {
                BoxType(*b)
            }
        }

        impl PartialEq<&str> for BoxType {
            fn eq(&self, other: &&str) -> bool {
                self.as_str() == *other
            }
        }

        /// Parsed header of a single ISOBMFF box.
        //
        //  ┌────────────┬──────────┬──────────────────────────┐
        //  │ size32 (4) │ type (4) │ [largesize (8) if size=1] │
        //  └────────────┴──────────┴──────────────────────────┘
        #[derive(Debug, Clone)]
        pub struct BoxHeader {
            pub offset:      u64,
            pub total_size:  u64,
            pub box_type:    BoxType,
            pub uuid:        Option<[u8; 16]>,
            pub header_size: u64
        }

        impl BoxHeader {
            pub fn payload_offset(&self) -> u64 {
                self.offset + self.header_size
            }
            pub fn payload_size(&self) -> u64 {
                self.total_size.saturating_sub(self.header_size)
            }
            pub fn end_offset(&self) -> u64 {
                self.offset + self.total_size
            }

            pub fn read<R: Read + Seek>(
                reader: &mut IsobmffReader<R>, container_end: u64
            ) -> Result<Option<Self>> {
                let offset = reader.pos()?;
                if offset + 8 > container_end {
                    return Ok(None);
                }

                let size32 = reader.read_u32_be()?;
                let type_bytes = reader.read_4cc()?;

                for &b in &type_bytes {
                    if !b.is_ascii_graphic() && b != b' ' {
                        return Ok(None);
                    }
                }

                let box_type = BoxType(type_bytes);
                let mut header_size: u64 = 8;

                let total_size: u64 = match size32 {
                    0 => container_end - offset,
                    1 => {
                        if offset + 16 > container_end {
                            return Err(Error::UnexpectedEof { offset, needed: 16 });
                        }
                        header_size = 16;
                        reader.read_u64_be()?
                    }
                    s => s as u64
                };

                if total_size < header_size {
                    return Err(Error::InvalidBoxSize {
                        offset,
                        size: total_size
                    });
                }
                if offset + total_size > container_end {
                    return Err(Error::InvalidBoxSize {
                        offset,
                        size: total_size
                    });
                }

                let uuid = if box_type == "uuid" {
                    if offset + header_size + 16 > container_end {
                        return Err(Error::UnexpectedEof {
                            offset,
                            needed: header_size + 16
                        });
                    }
                    let mut u = [0u8; 16];
                    reader.inner_mut().read_exact(&mut u)?;
                    header_size += 16;
                    Some(u)
                } else {
                    None
                };

                Ok(Some(BoxHeader {
                    offset,
                    total_size,
                    box_type,
                    uuid,
                    header_size
                }))
            }
        }
    }

    // ── fullbox ───────────────────────────────────────────────────────────────
    pub mod fullbox {
        use std::io::{Read, Seek};

        use super::error::Result;
        use super::reader::IsobmffReader;

        /// ISO 14496-12 §4.2 — version(1) + flags(3) prefix on FullBox subtypes.
        pub const FULLBOX_HEADER_SIZE: u64 = 4;

        #[derive(Debug, Clone, Copy)]
        pub struct FullBoxHeader {
            pub version: u8,
            pub flags:   u32 // 24 meaningful bits
        }

        impl FullBoxHeader {
            pub fn read<R: Read + Seek>(reader: &mut IsobmffReader<R>) -> Result<Self> {
                let version = reader.read_u8()?;
                let b0 = reader.read_u8()? as u32;
                let b1 = reader.read_u8()? as u32;
                let b2 = reader.read_u8()? as u32;
                Ok(FullBoxHeader {
                    version,
                    flags: (b0 << 16) | (b1 << 8) | b2
                })
            }

            pub fn has_flag(&self, flag: u32) -> bool {
                self.flags & flag != 0
            }
        }
    }

    // ── tree ──────────────────────────────────────────────────────────────────
    pub mod tree {
        use std::io::{Read, Seek};

        use super::error::Result;
        use super::header::BoxHeader;
        use super::reader::IsobmffReader;

        /// A single node in the lazy box tree.
        /// Payloads are NOT read during tree construction — only (offset, size).
        #[derive(Debug, Clone)]
        pub struct RawBox {
            pub header:   BoxHeader,
            pub children: Vec<RawBox>
        }

        impl RawBox {
            pub fn box_type(&self) -> &str {
                self.header.box_type.as_str()
            }
            pub fn payload_offset(&self) -> u64 {
                self.header.payload_offset()
            }
            pub fn payload_size(&self) -> u64 {
                self.header.payload_size()
            }

            pub fn child(&self, box_type: &str) -> Option<&RawBox> {
                self.children.iter().find(|c| c.box_type() == box_type)
            }

            pub fn children_of_type(&self, box_type: &str) -> Vec<&RawBox> {
                self.children
                    .iter()
                    .filter(|c| c.box_type() == box_type)
                    .collect()
            }

            pub fn walk<F: FnMut(&RawBox)>(&self, f: &mut F) {
                f(self);
                for child in &self.children {
                    child.walk(f);
                }
            }

            pub fn find(&self, box_type: &str) -> Option<&RawBox> {
                if self.box_type() == box_type {
                    return Some(self);
                }
                for child in &self.children {
                    if let Some(found) = child.find(box_type) {
                        return Some(found);
                    }
                }
                None
            }

            pub fn find_all<'a>(&'a self, box_type: &str, out: &mut Vec<&'a RawBox>) {
                if self.box_type() == box_type {
                    out.push(self);
                }
                for child in &self.children {
                    child.find_all(box_type, out);
                }
            }
        }

        /// The parsed box tree for an entire file.
        pub struct BoxTree {
            pub roots: Vec<RawBox>
        }

        impl BoxTree {
            pub fn root(&self, box_type: &str) -> Option<&RawBox> {
                self.roots.iter().find(|b| b.box_type() == box_type)
            }

            pub fn find(&self, box_type: &str) -> Option<&RawBox> {
                for root in &self.roots {
                    if let Some(found) = root.find(box_type) {
                        return Some(found);
                    }
                }
                None
            }

            pub fn find_all(&self, box_type: &str) -> Vec<&RawBox> {
                let mut out = Vec::new();
                for root in &self.roots {
                    root.find_all(box_type, &mut out);
                }
                out
            }
        }

        pub type IsContainerFn = fn(&str) -> bool;

        /// Standard ISOBMFF container boxes whose payloads contain more boxes.
        pub fn is_standard_container(box_type: &str) -> bool {
            matches!(
                box_type,
                "moov"
                    | "trak"
                    | "mdia"
                    | "minf"
                    | "dinf"
                    | "stbl"
                    | "mvex"
                    | "moof"
                    | "traf"
                    | "mfra"
                    | "udta"
                    | "edts"
                    | "meta"
                    | "iprp"
                    | "ipco"
                    | "iref"
                    | "iinf"
                    | "grpl"
                    | "meco"
                    | "mere"
                    | "strk"
                    | "sinf"
                    | "schi"
            )
        }

        /// Parse all boxes from [start, start+length) in the reader.
        pub fn parse_boxes<R: Read + Seek>(
            reader: &mut IsobmffReader<R>, start: u64, length: u64, is_container: IsContainerFn
        ) -> Result<Vec<RawBox>> {
            let mut boxes = Vec::new();
            let end = start + length;
            let mut pos = start;


            while pos + 8 <= end {
                println!("Box type: {}",pos);
                reader.seek_to(pos)?;
                let header = match BoxHeader::read(reader, end)? {
                    Some(h) => h,
                    None => break
                };
                let next_pos = header.end_offset();
                let box_str = header.box_type.as_str();

                let children = if is_container(header.box_type.as_str()) {
                    // Properly handle FullBox container headers for meta, iref, and iinf
                    let child_start = match box_str {
                        "meta" | "iref" => header.payload_offset() + 4,
                        "iinf" => {
                            reader.seek_to(header.payload_offset())?;
                            let version = reader.read_u8()?;
                            if version == 0 {
                                header.payload_offset() + 6
                            } else {
                                header.payload_offset() + 8
                            }
                        }
                        _ => header.payload_offset()
                    };

                    let child_len = (header.offset + header.total_size).saturating_sub(child_start);
                    parse_boxes(reader, child_start, child_len, is_container)?
                } else {
                    Vec::new()
                };

                if box_str == "idat"{
                    let start = reader.total_len;
                    let c = reader.pos().unwrap();
                    println!("Idat found");
                }
                boxes.push(RawBox { header, children });
                pos = next_pos;
            }
            Ok(boxes)
        }

        /// Convenience: parse an entire file into a BoxTree.
        pub fn parse_file<R: Read + Seek>(
            reader: &mut IsobmffReader<R>, is_container: IsContainerFn
        ) -> Result<BoxTree> {
            let total = reader.total_len;
            let roots = parse_boxes(reader, 0, total, is_container)?;
            Ok(BoxTree { roots })
        }
    }

    // ── visitor ───────────────────────────────────────────────────────────────
    pub mod visitor {
        use std::io::{Read, Seek};

        use super::error::Result;
        use super::reader::IsobmffReader;
        use super::tree::RawBox;

        /// Implement this to walk and decode the box tree without modifying the parser.
        pub trait BoxVisitor {
            /// Return `Ok(true)` to recurse into children, `Ok(false)` to skip.
            fn visit<R: Read + Seek>(
                &mut self, raw: &RawBox, reader: &mut IsobmffReader<R>
            ) -> Result<bool>;

            fn visit_tree<R: Read + Seek>(
                &mut self, roots: &[RawBox], reader: &mut IsobmffReader<R>
            ) -> Result<()> {
                for root in roots {
                    self.visit_node(root, reader)?;
                }
                Ok(())
            }

            fn visit_node<R: Read + Seek>(
                &mut self, node: &RawBox, reader: &mut IsobmffReader<R>
            ) -> Result<()> {
                if self.visit(node, reader)? {
                    for child in &node.children {
                        self.visit_node(child, reader)?;
                    }
                }
                Ok(())
            }
        }
    }

    // ── boxes ─────────────────────────────────────────────────────────────────
    pub mod boxes {
        use std::io::Cursor;

        use super::error::{Error, Result};
        use super::reader::{fourcc_str, u16_be_at, u32_be_at, u64_be_at, u8_at, IsobmffReader};

        // ── ftyp — File Type Box (ISO 14496-12 §4.3) ──────────────────────────

        #[derive(Debug, Clone)]
        pub struct FtypBox {
            pub major_brand:       String,
            pub minor_version:     u32,
            pub compatible_brands: Vec<String>
        }

        impl FtypBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 8 {
                    return Err(Error::PayloadTooShort {
                        box_type: "ftyp".into(),
                        needed:   8,
                        have:     payload.len()
                    });
                }
                Ok(FtypBox {
                    major_brand:       fourcc_str(&[
                        payload[0], payload[1], payload[2], payload[3]
                    ]),
                    minor_version:     u32_be_at(payload, 4),
                    compatible_brands: payload[8..]
                        .chunks(4)
                        .filter(|c| c.len() == 4)
                        .map(|c| fourcc_str(&[c[0], c[1], c[2], c[3]]))
                        .collect()
                })
            }
        }

        // ── hdlr — Handler Reference Box (ISO 14496-12 §8.4.3) ───────────────

        #[derive(Debug, Clone)]
        pub struct HdlrBox {
            pub version:      u8,
            pub handler_type: String,
            pub name:         String
        }

        impl HdlrBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 24 {
                    return Err(Error::PayloadTooShort {
                        box_type: "hdlr".into(),
                        needed:   24,
                        have:     payload.len()
                    });
                }
                let version = u8_at(payload, 0);
                let handler_type = fourcc_str(&[payload[8], payload[9], payload[10], payload[11]]);
                let name_bytes = &payload[24..];
                let null_pos = name_bytes
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(name_bytes.len());
                Ok(HdlrBox {
                    version,
                    handler_type,
                    name: String::from_utf8_lossy(&name_bytes[..null_pos]).into_owned()
                })
            }
        }

        // ── pitm — Primary Item Box (ISO 14496-12 §8.11.4) ───────────────────

        #[derive(Debug, Clone)]
        pub struct PitmBox {
            pub version: u8,
            pub item_id: u32
        }

        impl PitmBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 4 {
                    return Err(Error::PayloadTooShort {
                        box_type: "pitm".into(),
                        needed:   4,
                        have:     payload.len()
                    });
                }
                let version = u8_at(payload, 0);
                let item_id = if version == 0 {
                    if payload.len() < 6 {
                        return Err(Error::PayloadTooShort {
                            box_type: "pitm".into(),
                            needed:   6,
                            have:     payload.len()
                        });
                    }
                    u16_be_at(payload, 4) as u32
                } else {
                    if payload.len() < 8 {
                        return Err(Error::PayloadTooShort {
                            box_type: "pitm".into(),
                            needed:   8,
                            have:     payload.len()
                        });
                    }
                    u32_be_at(payload, 4)
                };
                Ok(PitmBox { version, item_id })
            }
        }

        // ── infe — Item Info Entry (ISO 14496-12 §8.11.6) ────────────────────

        #[derive(Debug, Clone)]
        pub struct InfeBox {
            pub version:               u8,
            pub item_id:               u32,
            pub item_protection_index: u16,
            pub item_type:             Option<String>,
            pub item_name:             String,
            pub content_type:          Option<String>,
            pub content_encoding:      Option<String>
        }

        impl InfeBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 4 {
                    return Err(Error::PayloadTooShort {
                        box_type: "infe".into(),
                        needed:   4,
                        have:     payload.len()
                    });
                }
                let version = u8_at(payload, 0);
                let mut cursor = Cursor::new(payload);
                let mut r = IsobmffReader::new(&mut cursor).map_err(|e| Error::ParseError {
                    box_type: "infe".into(),
                    msg:      e.to_string()
                })?;
                let _ = r.read_u32_be()?; // skip version+flags

                let (
                    item_id,
                    item_protection_index,
                    item_type,
                    item_name,
                    content_type,
                    content_encoding
                );

                if version == 0 || version == 1 {
                    item_id = r.read_u16_be()? as u32;
                    item_protection_index = r.read_u16_be()?;
                    item_name = r.read_null_terminated_string(256)?;
                    content_type = Some(r.read_null_terminated_string(256)?);
                    content_encoding = Some(r.read_null_terminated_string(256)?);
                    item_type = None;
                } else {
                    item_id = if version == 2 { r.read_u16_be()? as u32 } else { r.read_u32_be()? };
                    item_protection_index = r.read_u16_be()?;
                    let type_bytes = r.read_4cc()?;
                    item_type = Some(fourcc_str(&type_bytes));
                    item_name = r.read_null_terminated_string(256)?;
                    content_type = None;
                    content_encoding = None;
                }

                Ok(InfeBox {
                    version,
                    item_id,
                    item_protection_index,
                    item_type,
                    item_name,
                    content_type,
                    content_encoding
                })
            }
        }

        // ── iinf — Item Information Box ───────────────────────────────────────

        #[derive(Debug, Clone)]
        pub struct IinfBox {
            pub version:     u8,
            pub entry_count: u32
        }

        impl IinfBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 6 {
                    return Err(Error::PayloadTooShort {
                        box_type: "iinf".into(),
                        needed:   6,
                        have:     payload.len()
                    });
                }
                let version = u8_at(payload, 0);
                let entry_count =
                    if version == 0 { u16_be_at(payload, 4) as u32 } else { u32_be_at(payload, 4) };
                Ok(IinfBox {
                    version,
                    entry_count
                })
            }
        }

        // ── iloc — Item Location Box (ISO 14496-12 §8.11.3) ──────────────────

        #[derive(Debug, Clone)]
        pub struct IlocExtent {
            pub extent_index:  u64,
            pub extent_offset: u64,
            pub extent_length: u64
        }

        #[derive(Debug, Clone)]
        pub struct IlocItem {
            pub item_id:              u32,
            pub construction_method:  u8,
            pub data_reference_index: u16,
            pub base_offset:          u64,
            pub extents:              Vec<IlocExtent>
        }

        #[derive(Debug, Clone)]
        pub struct IlocBox {
            pub version:          u8,
            pub offset_size:      u8,
            pub length_size:      u8,
            pub base_offset_size: u8,
            pub index_size:       u8,
            pub items:            Vec<IlocItem>
        }

        impl IlocBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 8 {
                    return Err(Error::PayloadTooShort {
                        box_type: "iloc".into(),
                        needed:   8,
                        have:     payload.len()
                    });
                }
                let version = u8_at(payload, 0);
                let offset_size = (u8_at(payload, 4) >> 4) & 0x0F;
                let length_size = u8_at(payload, 4) & 0x0F;
                let base_offset_size = (u8_at(payload, 5) >> 4) & 0x0F;
                let index_size = if version >= 1 { u8_at(payload, 5) & 0x0F } else { 0 };

                let item_count: u32 = if version < 2 {
                    u16_be_at(payload, 6) as u32
                } else {
                    if payload.len() < 10 {
                        return Err(Error::PayloadTooShort {
                            box_type: "iloc".into(),
                            needed:   10,
                            have:     payload.len()
                        });
                    }
                    u32_be_at(payload, 6)
                };

                let mut pos = if version < 2 { 8usize } else { 10 };
                let mut items = Vec::with_capacity(item_count as usize);

                for _ in 0..item_count {
                    let item_id = if version < 2 {
                        let v = u16_be_at(payload, pos) as u32;
                        pos += 2;
                        v
                    } else {
                        let v = u32_be_at(payload, pos);
                        pos += 4;
                        v
                    };
                    let construction_method = if version >= 1 {
                        let v = (u16_be_at(payload, pos) & 0x0F) as u8;
                        pos += 2;
                        v
                    } else {
                        0
                    };
                    let data_reference_index = u16_be_at(payload, pos);
                    pos += 2;
                    let base_offset = read_sized(payload, &mut pos, base_offset_size)?;
                    let extent_count = u16_be_at(payload, pos) as usize;
                    pos += 2;
                    let mut extents = Vec::with_capacity(extent_count);
                    for _ in 0..extent_count {
                        let extent_index = if version >= 1 && index_size > 0 {
                            read_sized(payload, &mut pos, index_size)?
                        } else {
                            0
                        };
                        let extent_offset = read_sized(payload, &mut pos, offset_size)?;
                        let extent_length = read_sized(payload, &mut pos, length_size)?;
                        extents.push(IlocExtent {
                            extent_index,
                            extent_offset,
                            extent_length
                        });
                    }
                    items.push(IlocItem {
                        item_id,
                        construction_method,
                        data_reference_index,
                        base_offset,
                        extents
                    });
                }

                Ok(IlocBox {
                    version,
                    offset_size,
                    length_size,
                    base_offset_size,
                    index_size,
                    items
                })
            }
        }

        fn read_sized(buf: &[u8], pos: &mut usize, size: u8) -> Result<u64> {
            let v = match size {
                0 => 0u64,
                4 => {
                    if buf.len() < *pos + 4 {
                        return Err(Error::PayloadTooShort {
                            box_type: "iloc".into(),
                            needed:   *pos + 4,
                            have:     buf.len()
                        });
                    }
                    u32_be_at(buf, *pos) as u64
                }
                8 => {
                    if buf.len() < *pos + 8 {
                        return Err(Error::PayloadTooShort {
                            box_type: "iloc".into(),
                            needed:   *pos + 8,
                            have:     buf.len()
                        });
                    }
                    u64_be_at(buf, *pos)
                }
                _ => {
                    return Err(Error::ParseError {
                        box_type: "iloc".into(),
                        msg:      format!("unsupported size field: {}", size)
                    });
                }
            };
            *pos += size as usize;
            Ok(v)
        }

        // ── ipma — Item Property Association Box (ISO 14496-12 §8.11.14) ─────

        #[derive(Debug, Clone)]
        pub struct IpmaEntry {
            pub item_id:      u32,
            /// (property_index, essential) — index is 1-based into ipco children
            pub associations: Vec<(u16, bool)>
        }

        #[derive(Debug, Clone)]
        pub struct IpmaBox {
            pub version: u8,
            pub flags:   u32,
            pub entries: Vec<IpmaEntry>
        }

        impl IpmaBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 8 {
                    return Err(Error::PayloadTooShort {
                        box_type: "ipma".into(),
                        needed:   8,
                        have:     payload.len()
                    });
                }
                let version = u8_at(payload, 0);
                let flags = ((u8_at(payload, 1) as u32) << 16)
                    | ((u8_at(payload, 2) as u32) << 8)
                    | (u8_at(payload, 3) as u32);
                let entry_count = u32_be_at(payload, 4);
                let mut pos = 8usize;
                let large_item_id = version >= 1;
                let large_prop_idx = flags & 1 != 0;
                let mut entries = Vec::with_capacity(entry_count as usize);

                for _ in 0..entry_count {
                    let item_id = if large_item_id {
                        let v = u32_be_at(payload, pos);
                        pos += 4;
                        v
                    } else {
                        let v = u16_be_at(payload, pos) as u32;
                        pos += 2;
                        v
                    };
                    let assoc_count = u8_at(payload, pos) as usize;
                    pos += 1;
                    let mut associations = Vec::with_capacity(assoc_count);
                    for _ in 0..assoc_count {
                        if large_prop_idx {
                            let raw = u16_be_at(payload, pos);
                            pos += 2;
                            associations.push((raw & 0x7FFF, (raw >> 15) != 0));
                        } else {
                            let raw = u8_at(payload, pos);
                            pos += 1;
                            associations.push(((raw & 0x7F) as u16, (raw >> 7) != 0));
                        }
                    }
                    entries.push(IpmaEntry {
                        item_id,
                        associations
                    });
                }
                Ok(IpmaBox {
                    version,
                    flags,
                    entries
                })
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// mod heif
// ══════════════════════════════════════════════════════════════════════════════
mod heif {

    // ── boxes ─────────────────────────────────────────────────────────────────
    pub mod boxes {
        use crate::isobmff::error::{Error, Result};
        use crate::isobmff::reader::{fourcc_str, u16_be_at, u32_be_at, u8_at};

        // ── ispe — Image Spatial Extents (ISO 23008-12 §6.5.3) ───────────────

        #[derive(Debug, Clone)]
        pub struct IspeBox {
            pub image_width:  u32,
            pub image_height: u32
        }

        impl IspeBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 12 {
                    return Err(Error::PayloadTooShort {
                        box_type: "ispe".into(),
                        needed:   12,
                        have:     payload.len()
                    });
                }
                Ok(IspeBox {
                    image_width:  u32_be_at(payload, 4),
                    image_height: u32_be_at(payload, 8)
                })
            }
        }

        // ── irot — Image Rotation (ISO 23008-12 §6.5.10) ─────────────────────

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Rotation {
            Degrees0,
            Degrees90,
            Degrees180,
            Degrees270
        }

        impl Rotation {
            pub fn degrees(self) -> u16 {
                match self {
                    Rotation::Degrees0 => 0,
                    Rotation::Degrees90 => 90,
                    Rotation::Degrees180 => 180,
                    Rotation::Degrees270 => 270
                }
            }
        }

        #[derive(Debug, Clone)]
        pub struct IrotBox {
            pub rotation: Rotation
        }

        impl IrotBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.is_empty() {
                    return Err(Error::PayloadTooShort {
                        box_type: "irot".into(),
                        needed:   1,
                        have:     0
                    });
                }
                Ok(IrotBox {
                    rotation: match u8_at(payload, 0) & 0x03 {
                        0 => Rotation::Degrees0,
                        1 => Rotation::Degrees90,
                        2 => Rotation::Degrees180,
                        _ => Rotation::Degrees270
                    }
                })
            }
        }

        // ── imir — Image Mirror (ISO 23008-12 §6.5.12) ───────────────────────

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum MirrorAxis {
            Vertical,
            Horizontal
        }

        #[derive(Debug, Clone)]
        pub struct ImirBox {
            pub axis: MirrorAxis
        }

        impl ImirBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.is_empty() {
                    return Err(Error::PayloadTooShort {
                        box_type: "imir".into(),
                        needed:   1,
                        have:     0
                    });
                }
                Ok(ImirBox {
                    axis: if u8_at(payload, 0) & 1 == 0 {
                        MirrorAxis::Vertical
                    } else {
                        MirrorAxis::Horizontal
                    }
                })
            }
        }

        // ── colr — Colour Information ─────────────────────────────────────────

        #[derive(Debug, Clone)]
        pub enum ColourInfo {
            Nclx {
                colour_primaries:         u16,
                transfer_characteristics: u16,
                matrix_coefficients:      u16,
                full_range:               bool
            },
            Icc {
                profile_type: String,
                icc_data:     Vec<u8>
            }
        }

        #[derive(Debug, Clone)]
        pub struct ColrBox {
            pub colour_info: ColourInfo
        }

        impl ColrBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 4 {
                    return Err(Error::PayloadTooShort {
                        box_type: "colr".into(),
                        needed:   4,
                        have:     payload.len()
                    });
                }
                let colour_type = fourcc_str(&[payload[0], payload[1], payload[2], payload[3]]);
                let colour_info = match colour_type.as_str() {
                    "nclx" => {
                        if payload.len() < 11 {
                            return Err(Error::PayloadTooShort {
                                box_type: "colr".into(),
                                needed:   11,
                                have:     payload.len()
                            });
                        }
                        ColourInfo::Nclx {
                            colour_primaries:         u16_be_at(payload, 4),
                            transfer_characteristics: u16_be_at(payload, 6),
                            matrix_coefficients:      u16_be_at(payload, 8),
                            full_range:               (u8_at(payload, 10) >> 7) != 0
                        }
                    }
                    "rICC" | "prof" => ColourInfo::Icc {
                        profile_type: colour_type,
                        icc_data:     payload[4..].to_vec()
                    },
                    other => {
                        return Err(Error::ParseError {
                            box_type: "colr".into(),
                            msg:      format!("unknown colour_type '{}'", other)
                        });
                    }
                };
                Ok(ColrBox { colour_info })
            }
        }

        // ── auxC — Auxiliary Type Property (ISO 23008-12 §6.5.8) ─────────────

        #[derive(Debug, Clone)]
        pub struct AuxCBox {
            pub aux_type:    String,
            pub aux_subtype: Vec<u8>
        }

        impl AuxCBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 5 {
                    return Err(Error::PayloadTooShort {
                        box_type: "auxC".into(),
                        needed:   5,
                        have:     payload.len()
                    });
                }
                let after = &payload[4..];
                let null_pos = after.iter().position(|&b| b == 0).unwrap_or(after.len());
                Ok(AuxCBox {
                    aux_type:    String::from_utf8_lossy(&after[..null_pos]).into_owned(),
                    aux_subtype: if null_pos + 1 < after.len() {
                        after[null_pos + 1..].to_vec()
                    } else {
                        Vec::new()
                    }
                })
            }
        }

        // ── hvcC — HEVC Decoder Config (ISO 14496-15 §8.3.3) ─────────────────

        #[derive(Debug, Clone)]
        pub struct HvcCBox {
            pub raw:                 Vec<u8>,
            pub general_profile_idc: u8,
            pub general_level_idc:   u8
        }

        impl HvcCBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 23 {
                    return Err(Error::PayloadTooShort {
                        box_type: "hvcC".into(),
                        needed:   23,
                        have:     payload.len()
                    });
                }
                Ok(HvcCBox {
                    raw:                 payload.to_vec(),
                    general_profile_idc: payload[2] & 0x1F,
                    general_level_idc:   payload[12]
                })
            }
        }

        // ── av1C — AV1 Codec Config ───────────────────────────────────────────

        #[derive(Debug, Clone)]
        pub struct Av1CBox {
            pub seq_profile:     u8,
            pub seq_level_idx_0: u8,
            pub raw:             Vec<u8>
        }

        impl Av1CBox {
            pub fn parse(payload: &[u8]) -> Result<Self> {
                if payload.len() < 4 {
                    return Err(Error::PayloadTooShort {
                        box_type: "av1C".into(),
                        needed:   4,
                        have:     payload.len()
                    });
                }
                Ok(Av1CBox {
                    seq_profile:     (payload[1] >> 5) & 0x07,
                    seq_level_idx_0: payload[1] & 0x1F,
                    raw:             payload.to_vec()
                })
            }
        }

        // ── iref entry ────────────────────────────────────────────────────────

        #[derive(Debug, Clone)]
        pub struct ItemReference {
            pub reference_type: String,
            pub from_item_id:   u32,
            pub to_item_ids:    Vec<u32>
        }

        impl ItemReference {
            pub fn parse(box_type: &str, payload: &[u8], iref_version: u8) -> Result<Self> {
                if payload.len() < 4 {
                    return Err(Error::PayloadTooShort {
                        box_type: box_type.to_string(),
                        needed:   4,
                        have:     payload.len()
                    });
                }
                let mut pos = 0usize;
                let from_item_id = if iref_version == 0 {
                    let v = u16_be_at(payload, pos) as u32;
                    pos += 2;
                    v
                } else {
                    let v = u32_be_at(payload, pos);
                    pos += 4;
                    v
                };
                let ref_count = u16_be_at(payload, pos) as usize;
                pos += 2;
                let mut to_item_ids = Vec::with_capacity(ref_count);
                for _ in 0..ref_count {
                    let id = if iref_version == 0 {
                        let v = u16_be_at(payload, pos) as u32;
                        pos += 2;
                        v
                    } else {
                        let v = u32_be_at(payload, pos);
                        pos += 4;
                        v
                    };
                    to_item_ids.push(id);
                }
                Ok(ItemReference {
                    reference_type: box_type.to_string(),
                    from_item_id,
                    to_item_ids
                })
            }
        }

        // ── grid — Derived Image Grid (ISO 23008-12 §6.6.2) ──────────────────
        //
        // The `grid` item type is how HEIC stores large images: it tiles them
        // into a rows×columns grid of equal-sized HEVC tiles.  The grid payload
        // is stored in the item's data (via iloc extents) — NOT in a box header.
        //
        //  version(1) flags(1) rows_minus_one(1) columns_minus_one(1)
        //  output_width (2 or 4 bytes depending on flags bit 0)
        //  output_height(2 or 4 bytes)
        //
        // `flags & 1` == 0 → widths are u16; == 1 → widths are u32.

        #[derive(Debug, Clone)]
        pub struct GridBox {
            /// Number of tile columns (columns_minus_one + 1)
            pub columns:       u32,
            /// Number of tile rows (rows_minus_one + 1)
            pub rows:          u32,
            /// Final composited output width in pixels
            pub output_width:  u32,
            /// Final composited output height in pixels
            pub output_height: u32
        }

        impl GridBox {
            /// Parse from the raw item data bytes (not a box payload — no size/type header).
            pub fn parse(data: &[u8]) -> Result<Self> {
                if data.len() < 8 {
                    return Err(Error::PayloadTooShort {
                        box_type: "grid".into(),
                        needed:   8,
                        have:     data.len()
                    });
                }
                // version = data[0] (must be 0)
                let flags = u8_at(data, 1);
                let rows_minus_one = u8_at(data, 2);
                let cols_minus_one = u8_at(data, 3);
                let large = flags & 1 != 0;
                let (output_width, output_height) = if large {
                    if data.len() < 12 {
                        return Err(Error::PayloadTooShort {
                            box_type: "grid".into(),
                            needed:   12,
                            have:     data.len()
                        });
                    }
                    (u32_be_at(data, 4), u32_be_at(data, 8))
                } else {
                    (u16_be_at(data, 4) as u32, u16_be_at(data, 6) as u32)
                };
                Ok(GridBox {
                    columns: cols_minus_one as u32 + 1,
                    rows: rows_minus_one as u32 + 1,
                    output_width,
                    output_height
                })
            }
        }
    }

    // ── properties ────────────────────────────────────────────────────────────
    pub mod properties {
        use super::boxes::{AuxCBox, Av1CBox, ColrBox, HvcCBox, ImirBox, IrotBox, IspeBox};

        /// All decoded properties that may apply to a HEIF image item.
        #[derive(Debug, Clone, Default)]
        pub struct ItemProperties {
            pub width:                  Option<u32>,
            pub height:                 Option<u32>,
            pub rotation:               Option<u32>, // degrees: 0, 90, 180, 270
            pub mirror:                 Option<String>,
            pub colour_info:            Option<String>,
            pub aux_type:               Option<String>,
            pub hevc_profile:           Option<u8>,
            pub hevc_level:             Option<u8>,
            pub av1_profile:            Option<u8>,
            pub av1_level:              Option<u8>,
            pub applied_property_types: Vec<String>,

            // ADD THIS LINE:
            pub hevc_config: Option<Vec<u8>>
        }

        impl ItemProperties {
            pub fn apply_ispe(&mut self, b: &IspeBox) {
                self.width = Some(b.image_width);
                self.height = Some(b.image_height);
                self.applied_property_types.push("ispe".into());
            }
            pub fn apply_irot(&mut self, b: &IrotBox) {
                self.rotation = Some(b.rotation.degrees() as u32);
                self.applied_property_types.push("irot".into());
            }
            pub fn apply_imir(&mut self, b: &ImirBox) {
                self.mirror = Some(format!("{:?}", b.axis));
                self.applied_property_types.push("imir".into());
            }
            pub fn apply_colr(&mut self, b: &ColrBox) {
                self.colour_info = Some(format!("{:?}", b.colour_info));
                self.applied_property_types.push("colr".into());
            }
            pub fn apply_auxc(&mut self, b: &AuxCBox) {
                self.aux_type = Some(b.aux_type.clone());
                self.applied_property_types.push("auxC".into());
            }
            // Update this line to save the raw bytes:
            pub fn apply_hvcc(&mut self, b: &HvcCBox) {
                self.hevc_profile = Some(b.general_profile_idc);
                self.hevc_level = Some(b.general_level_idc);
                self.hevc_config = Some(b.raw.clone()); // <--- ADD THIS
                self.applied_property_types.push("hvcC".into());
            }
            pub fn apply_av1c(&mut self, b: &Av1CBox) {
                self.av1_profile = Some(b.seq_profile);
                self.av1_level = Some(b.seq_level_idx_0);
                self.applied_property_types.push("av1C".into());
            }
        }
    }

    // ── file ──────────────────────────────────────────────────────────────────
    pub mod file {
        use std::collections::HashMap;
        use std::io::{Read, Seek};

        use super::boxes::{
            AuxCBox, Av1CBox, ColrBox, GridBox, HvcCBox, ImirBox, IrotBox, IspeBox, ItemReference
        };
        use super::properties::ItemProperties;
        use crate::isobmff::boxes::{FtypBox, IlocBox, InfeBox, IpmaBox, PitmBox};
        use crate::isobmff::error::{Error, Result};
        use crate::isobmff::reader::IsobmffReader;
        use crate::isobmff::tree::{is_standard_container, parse_file, BoxTree, RawBox};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum ItemKind {
            Image,
            Thumbnail,
            Auxiliary,
            Derived,
            Metadata,
            Other(String)
        }

        #[derive(Debug, Clone)]
        pub struct ImageItem {
            pub item_id:      u32,
            pub item_type:    String,
            pub item_name:    String,
            pub kind:         ItemKind,
            pub properties:   ItemProperties,
            /// (file_offset, byte_length) — concatenate extents for the full payload
            pub extents:      Vec<(u64, u64)>,
            pub derived_from: Vec<u32>,
            pub thumbnails:   Vec<u32>,
            pub auxiliary:    Vec<u32>,
            /// Decoded grid layout — Some only when item_type == "grid"
            pub grid:         Option<GridBox>
        }

        pub struct HeifFile {
            pub major_brand:       String,
            pub compatible_brands: Vec<String>,
            pub primary_item_id:   Option<u32>,
            pub items:             HashMap<u32, ImageItem>
        }

        impl HeifFile {
            pub fn primary_item(&self) -> Option<&ImageItem> {
                self.primary_item_id.and_then(|id| self.items.get(&id))
            }

            /// For a grid primary item, returns the first tile item, which carries
            /// the actual codec config (hvcC / av1C) and tile dimensions.
            pub fn first_tile_of(&self, item: &ImageItem) -> Option<&ImageItem> {
                item.derived_from.first().and_then(|id| self.items.get(id))
            }

            pub fn parse<R: Read + Seek>(source: R) -> Result<Self> {
                let mut reader = IsobmffReader::new(source)?;
                let tree = parse_file(&mut reader, is_standard_container)?;

                let ftyp = parse_ftyp(&tree, &mut reader)?;

                let meta = tree.root("meta").ok_or_else(|| Error::ParseError {
                    box_type: "meta".into(),
                    msg:      "missing 'meta' box".into()
                })?;

                let primary_item_id = parse_pitm(meta, &mut reader)?;
                let mut items = parse_iinf_items(meta, &mut reader)?;

                // NEW: Find the start of the 'idat' box payload
                let idat_offset = meta.child("idat");

                // Pass idat_offset into the iloc parser
                parse_iloc_extents(
                    meta,
                    &mut reader,
                    &mut items,
                    idat_offset.and_then(|x| Some(x.payload_offset()))
                )?;
                parse_iref(meta, &mut reader, &mut items)?;
                apply_item_properties(meta, &mut reader, &mut items)?;
                classify_items(&mut items);

                Ok(HeifFile {
                    major_brand: ftyp.major_brand,
                    compatible_brands: ftyp.compatible_brands,
                    primary_item_id,
                    items
                })
            }
        }

        // ── helpers ───────────────────────────────────────────────────────────

        fn read_payload<R: Read + Seek>(
            raw: &RawBox, reader: &mut IsobmffReader<R>
        ) -> Result<Vec<u8>> {
            reader.seek_to(raw.payload_offset())?;
            reader.read_bytes(raw.payload_size() as usize)
        }

        fn parse_ftyp<R: Read + Seek>(
            tree: &BoxTree, reader: &mut IsobmffReader<R>
        ) -> Result<FtypBox> {
            let raw = tree.root("ftyp").ok_or_else(|| Error::ParseError {
                box_type: "ftyp".into(),
                msg:      "missing".into()
            })?;
            let ftyp = FtypBox::parse(&read_payload(raw, reader)?)?;
            const HEIF_BRANDS: &[&str] = &[
                "heic", "heix", "hevc", "hevx", "heim", "heis", "hevm", "hevs", "mif1", "msf1",
                "avif", "avis", "MiHE", "MiHM"
            ];
            let is_heif = HEIF_BRANDS.contains(&ftyp.major_brand.trim())
                || ftyp
                    .compatible_brands
                    .iter()
                    .any(|b| HEIF_BRANDS.contains(&b.trim()));
            if !is_heif {
                eprintln!(
                    "[WARN] ftyp major_brand='{}' does not look like a HEIF file. Proceeding anyway.",
                    ftyp.major_brand
                );
            }
            Ok(ftyp)
        }

        fn parse_pitm<R: Read + Seek>(
            meta: &RawBox, reader: &mut IsobmffReader<R>
        ) -> Result<Option<u32>> {
            match meta.child("pitm") {
                Some(pitm) => Ok(Some(PitmBox::parse(&read_payload(pitm, reader)?)?.item_id)),
                None => Ok(None)
            }
        }

        fn parse_iinf_items<R: Read + Seek>(
            meta: &RawBox, reader: &mut IsobmffReader<R>
        ) -> Result<HashMap<u32, ImageItem>> {
            let mut items = HashMap::new();
            let iinf = match meta.child("iinf") {
                Some(b) => b,
                None => return Ok(items)
            };
            for infe_raw in iinf.children_of_type("infe") {
                match InfeBox::parse(&read_payload(infe_raw, reader)?) {
                    Ok(infe) => {
                        let item_type = infe.item_type.unwrap_or_else(|| "????".into());
                        items.insert(
                            infe.item_id,
                            ImageItem {
                                item_id: infe.item_id,
                                item_type,
                                item_name: infe.item_name,
                                kind: ItemKind::Other("pending".into()),
                                properties: ItemProperties::default(),
                                extents: Vec::new(),
                                derived_from: Vec::new(),
                                thumbnails: Vec::new(),
                                auxiliary: Vec::new(),
                                grid: None
                            }
                        );
                    }
                    Err(e) => eprintln!("[WARN] Failed to parse infe: {}", e)
                }
            }
            Ok(items)
        }

        fn parse_iloc_extents<R: Read + Seek>(
            meta: &RawBox,
            reader: &mut IsobmffReader<R>,
            items: &mut HashMap<u32, ImageItem>,
            idat_offset: Option<u64> // Add this parameter
        ) -> Result<()> {
            let iloc_raw = match meta.child("iloc") {
                Some(b) => b,
                None => return Ok(())
            };
            let iloc = IlocBox::parse(&read_payload(iloc_raw, reader)?)?;
            for iloc_item in &iloc.items {
                if let Some(item) = items.get_mut(&iloc_item.item_id) {
                    for ext in &iloc_item.extents {
                        // NEW: Resolve absolute file offset based on construction_method
                        let absolute_offset = match iloc_item.construction_method {
                            0 => iloc_item.base_offset + ext.extent_offset,
                            1 => {
                                if let Some(idat_start) = idat_offset {
                                    idat_start + iloc_item.base_offset + ext.extent_offset
                                } else {
                                    0 // Invalid state: construction_method 1 but no idat box
                                }
                            }
                            _ => 0 // method 2 is item-relative; ignoring for now
                        };

                        if absolute_offset > 0 {
                            item.extents.push((absolute_offset, ext.extent_length));
                        }
                    }

                    // If this is a grid item, read its data bytes and decode the grid layout.
                    if item.item_type == "grid" && !item.extents.is_empty() {
                        let (offset, length) = item.extents[0];
                        reader.seek_to(offset)?;
                        let data = reader.read_bytes(length.min(16) as usize)?;
                        match GridBox::parse(&data) {
                            Ok(g) => item.grid = Some(g),
                            Err(e) => eprintln!("[WARN] grid payload: {}", e)
                        }
                    }
                }
            }
            Ok(())
        }
        fn parse_iref<R: Read + Seek>(
            meta: &RawBox, reader: &mut IsobmffReader<R>, items: &mut HashMap<u32, ImageItem>
        ) -> Result<()> {
            let iref = match meta.child("iref") {
                Some(b) => b,
                None => return Ok(())
            };
            reader.seek_to(iref.payload_offset())?;
            let iref_version = reader.read_u8()?;
            for child in &iref.children {
                match ItemReference::parse(
                    child.box_type(),
                    &read_payload(child, reader)?,
                    iref_version
                ) {
                    Ok(r) => match r.reference_type.as_str() {
                        "dimg" => {
                            if let Some(item) = items.get_mut(&r.from_item_id) {
                                item.derived_from = r.to_item_ids;
                            }
                        }
                        "thmb" => {
                            if let Some(t) = items.get_mut(&r.from_item_id) {
                                t.kind = ItemKind::Thumbnail;
                            }
                            for &mid in &r.to_item_ids {
                                if let Some(m) = items.get_mut(&mid) {
                                    m.thumbnails.push(r.from_item_id);
                                }
                            }
                        }
                        "auxl" => {
                            if let Some(a) = items.get_mut(&r.from_item_id) {
                                a.kind = ItemKind::Auxiliary;
                            }
                            for &mid in &r.to_item_ids {
                                if let Some(m) = items.get_mut(&mid) {
                                    m.auxiliary.push(r.from_item_id);
                                }
                            }
                        }
                        _ => {}
                    },
                    Err(e) => eprintln!("[WARN] iref child '{}': {}", child.box_type(), e)
                }
            }
            Ok(())
        }

        fn apply_item_properties<R: Read + Seek>(
            meta: &RawBox, reader: &mut IsobmffReader<R>, items: &mut HashMap<u32, ImageItem>
        ) -> Result<()> {
            let iprp = match meta.child("iprp") {
                Some(b) => b,
                None => return Ok(())
            };
            let ipco = match iprp.child("ipco") {
                Some(b) => b,
                None => return Ok(())
            };
            let ipma_raw = match iprp.child("ipma") {
                Some(b) => b,
                None => return Ok(())
            };
            let ipma = IpmaBox::parse(&read_payload(ipma_raw, reader)?)?;
            let props: Vec<&RawBox> = ipco.children.iter().collect();
            for entry in &ipma.entries {
                let item = match items.get_mut(&entry.item_id) {
                    Some(i) => i,
                    None => continue
                };
                for &(prop_idx, _) in &entry.associations {
                    let prop = match props.get((prop_idx as usize).wrapping_sub(1)) {
                        Some(p) => p,
                        None => continue
                    };
                    let payload = match read_payload(prop, reader) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("[WARN] ipco: {}", e);
                            continue;
                        }
                    };
                    match prop.box_type() {
                        "ispe" => {
                            if let Ok(b) = IspeBox::parse(&payload) {
                                item.properties.apply_ispe(&b);
                            }
                        }
                        "irot" => {
                            if let Ok(b) = IrotBox::parse(&payload) {
                                item.properties.apply_irot(&b);
                            }
                        }
                        "imir" => {
                            if let Ok(b) = ImirBox::parse(&payload) {
                                item.properties.apply_imir(&b);
                            }
                        }
                        "colr" => {
                            if let Ok(b) = ColrBox::parse(&payload) {
                                item.properties.apply_colr(&b);
                            }
                        }
                        "auxC" => {
                            if let Ok(b) = AuxCBox::parse(&payload) {
                                item.properties.apply_auxc(&b);
                            }
                        }
                        "hvcC" => {
                            if let Ok(b) = HvcCBox::parse(&payload) {
                                item.properties.apply_hvcc(&b);
                            }
                        }
                        "av1C" => {
                            if let Ok(b) = Av1CBox::parse(&payload) {
                                item.properties.apply_av1c(&b);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(())
        }

        fn classify_items(items: &mut HashMap<u32, ImageItem>) {
            for item in items.values_mut() {
                if matches!(item.kind, ItemKind::Thumbnail | ItemKind::Auxiliary) {
                    continue;
                }
                item.kind = match item.item_type.as_str() {
                    "hvc1" | "hev1" | "av01" | "avc1" | "jpeg" | "j2k1" => ItemKind::Image,
                    "grid" | "iden" | "iovl" => ItemKind::Derived,
                    "Exif" | "mime" | "uri " => ItemKind::Metadata,
                    _ => ItemKind::Other(item.item_type.clone())
                };
            }
        }
    }
}

/// Helper to read the exact byte ranges for each item and write them to disk.
fn extract_items(heif: &HeifFile, input_path: &str, output_dir: &str) -> std::io::Result<()> {
    fs::create_dir_all(output_dir)?;
    let mut file = File::open(input_path)?;

    let mut ids: Vec<u32> = heif.items.keys().copied().collect();
    ids.sort();

    for id in ids {
        let item = &heif.items[&id];
        if item.extents.is_empty() {
            continue;
        }

        let is_hevc = item.item_type == "hvc1" || item.item_type == "hev1";
        let ext = if is_hevc { "hvc" } else { "bin" };
        let out_path = format!("{}/item_{:04}.{}", output_dir, id, ext);
        let mut out_file = File::create(&out_path)?;
        let mut total_bytes = 0;

        // --- HEVC HEADER INJECTION (VPS, SPS, PPS) ---
        let mut length_size = 4; // Default MP4 length prefix size
        if is_hevc {
            if let Some(ref hvc_c) = item.properties.hevc_config {
                if hvc_c.len() >= 23 {
                    // Extract lengthSizeMinusOne (byte 21, bottom 2 bits)
                    length_size = ((hvc_c[21] & 0x03) + 1) as usize;

                    let num_arrays = hvc_c[22];
                    let mut pos = 23;

                    // Parse the parameter set arrays and inject them with start codes
                    for _ in 0..num_arrays {
                        if pos >= hvc_c.len() {
                            break;
                        }
                        let num_nalus =
                            u16::from_be_bytes([hvc_c[pos + 1], hvc_c[pos + 2]]) as usize;
                        pos += 3;
                        for _ in 0..num_nalus {
                            if pos + 2 > hvc_c.len() {
                                break;
                            }
                            let nal_len = u16::from_be_bytes([hvc_c[pos], hvc_c[pos + 1]]) as usize;
                            pos += 2;
                            if pos + nal_len > hvc_c.len() {
                                break;
                            }

                            // Write Annex B start code + Parameter NALU
                            out_file.write_all(&[0, 0, 0, 1])?;
                            out_file.write_all(&hvc_c[pos..pos + nal_len])?;
                            total_bytes += 4 + nal_len;
                            pos += nal_len;
                        }
                    }
                }
            }
        }

        // --- PAYLOAD EXTRACTION & TRANSLATION ---
        for &(offset, length) in &item.extents {
            file.seek(SeekFrom::Start(offset))?;

            if is_hevc {
                // Read the whole chunk into memory so we can rewrite the prefixes
                let mut chunk_data = vec![0u8; length as usize];
                file.read_exact(&mut chunk_data)?;

                let mut pos = 0;
                while pos + length_size <= chunk_data.len() {
                    // Read the length prefix
                    let mut len_bytes = [0u8; 4];
                    for i in 0..length_size {
                        len_bytes[4 - length_size + i] = chunk_data[pos + i];
                    }
                    let nal_len = u32::from_be_bytes(len_bytes) as usize;
                    pos += length_size;

                    if pos + nal_len > chunk_data.len() {
                        break;
                    } // Safety catch

                    // Swap the length prefix for an Annex B start code
                    out_file.write_all(&[0, 0, 0, 1])?;
                    out_file.write_all(&chunk_data[pos..pos + nal_len])?;
                    total_bytes += 4 + nal_len;
                    pos += nal_len;
                }
            } else {
                // Non-HEVC items (Exif, mime, etc.) get copied raw
                let mut chunk = (&mut file).take(length);
                total_bytes += std::io::copy(&mut chunk, &mut out_file)? as usize;
            }
        }

        println!(
            "  Extracted item #{:<3} -> {} ({} bytes)",
            id, out_path, total_bytes
        );
    }

    Ok(())
}
fn print_item(item: &ImageItem) {
    println!("  type     : {}", item.item_type);
    println!("  kind     : {:?}", item.kind);
    if !item.item_name.is_empty() {
        println!("  name     : {}", item.item_name);
    }

    // Grid layout (only for grid items)
    if let Some(ref g) = item.grid {
        println!(
            "  grid     : {}×{} tiles → {}×{} px output",
            g.columns, g.rows, g.output_width, g.output_height
        );
    }

    let p = &item.properties;
    if let (Some(w), Some(h)) = (p.width, p.height) {
        println!("  size     : {}×{}", w, h);
    }
    if let Some(rot) = p.rotation {
        println!("  rotation : {}°", rot);
    }
    if let Some(ref m) = p.mirror {
        println!("  mirror   : {}", m);
    }
    // if let Some(ref c) = p.colour_info { println!("  colour   : {}", c); }
    if let Some(ref a) = p.aux_type {
        println!("  aux type : {}", a);
    }
    if let (Some(prof), Some(lvl)) = (p.hevc_profile, p.hevc_level) {
        println!("  HEVC     : profile={} level={}", prof, lvl);
    }
    if let (Some(prof), Some(lvl)) = (p.av1_profile, p.av1_level) {
        println!("  AV1      : profile={} level={}", prof, lvl);
    }
    if !p.applied_property_types.is_empty() {
        println!("  ipco     : [{}]", p.applied_property_types.join(", "));
    }
    if !item.extents.is_empty() {
        println!("  extents  :");
        for (off, len) in &item.extents {
            println!("    offset={} length={}", off, len);
        }
    }
    if !item.derived_from.is_empty() {
        println!(
            "  dimg→    : {}",
            item.derived_from
                .iter()
                .map(|i| format!("#{}", i))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !item.thumbnails.is_empty() {
        println!(
            "  thumbs   : {}",
            item.thumbnails
                .iter()
                .map(|i| format!("#{}", i))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !item.auxiliary.is_empty() {
        println!(
            "  aux items: {}",
            item.auxiliary
                .iter()
                .map(|i| format!("#{}", i))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CLI entry point
// ══════════════════════════════════════════════════════════════════════════════
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::{env, fs};

use heif::file::{HeifFile, ImageItem};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Use CLI arg if provided, otherwise fallback to your hardcoded path
    let path = if args.len() == 2 {
        args[1].clone()
    } else {
        String::from("/Users/etemesi/Downloads/IMG_4909.HEIC")
    };

    let file = File::open(&path).unwrap_or_else(|e| {
        eprintln!("Cannot open '{}': {}", path, e);
        std::process::exit(1);
    });

    let heif = HeifFile::parse(file).unwrap_or_else(|e| {
        eprintln!("Parse error: {}", e);
        std::process::exit(1);
    });

    println!("╔══════════════════════════════════════════╗");
    println!("║          HEIF Container Decoder          ║");
    println!("╚══════════════════════════════════════════╝");
    println!();
    println!("File         : {}", path);
    println!("Major brand  : {}", heif.major_brand);
    println!("Compatible   : {}", heif.compatible_brands.join(", "));
    if let Some(id) = heif.primary_item_id {
        println!("Primary item : #{}", id);
    }
    println!();

    if let Some(primary) = heif.primary_item() {
        println!(
            "━━━ Primary Image (item #{}) ━━━━━━━━━━━━━━━━",
            primary.item_id
        );
        print_item(primary);

        // For a grid primary, the codec lives on the tile items — follow dimg.
        if primary.item_type == "grid" {
            if let Some(tile) = heif.first_tile_of(primary) {
                println!();
                println!(
                    "  ┌─ tile codec (item #{}, type={}) ─────────",
                    tile.item_id, tile.item_type
                );
                let p = &tile.properties;
                if let (Some(w), Some(h)) = (p.width, p.height) {
                    println!("  │  tile size : {}×{}", w, h);
                }
                if let (Some(prof), Some(lvl)) = (p.hevc_profile, p.hevc_level) {
                    println!("  │  HEVC      : profile={} level={}", prof, lvl);
                }
                if let (Some(prof), Some(lvl)) = (p.av1_profile, p.av1_level) {
                    println!("  │  AV1       : profile={} level={}", prof, lvl);
                }
                println!(
                    "  │  tiles     : {} total ({} col × {} row)",
                    primary.derived_from.len(),
                    primary.grid.as_ref().map_or(0, |g| g.columns),
                    primary.grid.as_ref().map_or(0, |g| g.rows),
                );
                println!("  └──────────────────────────────────────────");
            }
        }
    }

    println!();
    println!(
        "━━━ All Items ({} total) ━━━━━━━━━━━━━━━━━━━━━",
        heif.items.len()
    );
    let mut ids: Vec<u32> = heif.items.keys().copied().collect();
    ids.sort();
    for id in ids {
        let item = &heif.items[&id];
        let marker = if heif.primary_item_id == Some(id) { " ◀ primary" } else { "" };
        println!();
        println!("  Item #{}{}", id, marker);
        print_item(item);
    }

    // NEW: Extract the payloads to the output directory
    let output_dir = "output_dirs";
    println!("\n━━━ Extracting Payloads to '{}' ━━━━━━━━━━━━", output_dir);

    if let Err(e) = extract_items(&heif, &path, output_dir) {
        eprintln!("Failed to extract items: {}", e);
    } else {
        println!("\nExtraction complete!");
    }
}
