use zune_core::bytestream::{ZByteReaderTrait, ZReader};

use crate::bmf_reader::FourCC;
use crate::errors::BmfErrors;

#[derive(Debug, Clone)]
pub struct FtypHeader {
    pub major_brand:       FourCC,
    pub minor_version:     u32,
    pub compatible_brands: Vec<FourCC>
}
pub struct MDatSection {
    pub(crate) raw_data:     Vec<u8>,
    pub(crate) start_offset: u64
}
#[derive(Debug, Clone, Default)]
pub struct MetaSection {
    pub hdlr:    Option<HdlrSection>,
    pub pitm:    Option<PitmSection>,
    pub iinf:    Option<IinfSection>,
    pub iloc:    Option<IlocSection>,
    pub iprp:    Option<IprpSection>,
    pub iref:    Option<IrefSection>,
    pub idat:    Option<IdatSection>
}
#[derive(Debug, Clone)]
pub struct HdlrSection {
    pub version:      u8,
    pub flags:        u32,
    pub handler_type: FourCC,
    pub name:         String
}

#[derive(Debug, Clone)]
pub struct PitmSection {
    pub version: u8,
    pub flags:   u32,
    pub item_id: u32 // The ID of the primary photo (e.g., the grid)
}

#[derive(Debug, Clone)]
pub struct IinfSection {
    pub version:     u8,
    pub flags:       u32,
    pub entry_count: u32,
    pub entries:     Vec<InfeSection>
}

#[derive(Debug, Clone)]
pub struct InfeSection {
    pub version:               u8,
    pub flags:                 u32,
    pub item_id:               u32,
    pub item_protection_index: u16,
    pub item_type:             FourCC, // e.g., "hvc1", "grid", "Exif"
    pub item_name:             String,

    // Only present in version 0 or 1
    pub content_type:     Option<String>,
    pub content_encoding: Option<String>,

    // Only present in version 3+ (rarely used, but in the spec)
    pub item_uri_type: Option<String>
}
#[derive(Debug, Clone)]
pub struct IlocSection {
    pub version:          u8,
    pub flags:            u32,
    pub offset_size:      u8, // Size in bytes of extent_offset (0, 4, or 8)
    pub length_size:      u8, // Size in bytes of extent_length (0, 4, or 8)
    pub base_offset_size: u8, // Size in bytes of base_offset (0, 4, or 8)
    pub index_size:       u8, // Size in bytes of extent_index (version >= 1 only)
    pub items:            Vec<IlocItem>
}

#[derive(Debug, Clone)]
pub struct IlocItem {
    pub item_id:              u32,
    pub construction_method:  u8, // 0 = file absolute, 1 = idat relative, 2 = item offset
    pub data_reference_index: u16, // Usually 0
    pub base_offset:          u64,
    pub extents:              Vec<IlocExtent>
}

#[derive(Debug, Clone)]
pub struct IlocExtent {
    pub extent_index:  Option<u64>,
    pub extent_offset: u64,
    pub extent_length: u64
}

#[derive(Debug, Clone)]
pub struct IrefSection {
    pub version:    u8,
    pub flags:      u32,
    pub references: Vec<IrefEntry>
}

#[derive(Debug, Clone)]
pub struct IrefEntry {
    pub reference_type: FourCC, // e.g., "dimg" (derived), "thmb" (thumbnail), "cdsc" (metadata)
    pub from_item_id:   u32,
    pub to_item_ids:    Vec<u32>
}

#[derive(Debug, Clone, Default)]
pub struct IprpSection {
    // Note: Iprp is a standard container, NO version or flags.
    pub ipco: Option<IpcoSection>,
    pub ipma: Option<IpmaSection>
}

// ── 6a. The Property Container (ipco) ────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct IpcoSection {
    // Ipco is a standard container. It holds a sequential list of property boxes.
    // Because there are dozens of property types, the most robust way to model
    // this in a strongly-typed system is an Enum that captures known properties
    // and falls back to a generic block for unknown ones.
    pub properties: Vec<ItemProperty>
}

#[derive(Debug, Clone)]
pub enum ItemProperty {
    Ispe {
        width:  u32,
        height: u32
    },
    Irot {
        angle_degrees: u16
    }, // decoded from (flags & 0x03) * 90
    Imir {
        axis: u8
    }, // 0 = vertical, 1 = horizontal
    Colr(ColourInformation), // Color information
    // nclx, rICC, or prof
    HvcC {
        payload: Vec<u8>
    },
    // HEVC decoder config
    Av1C {
        payload: Vec<u8>
    },
    // AV1 decoder config
    Pixi {
        channels:         u8,
        bits_per_channel: Vec<u8>
    },
    AuxC {
        aux_type: String,
        subtype:  Vec<u8>
    },
    Unknown {
        box_type: FourCC,
        payload:  Vec<u8>
    }
}

#[derive(Debug, Clone)]
pub struct IpmaSection {
    pub version: u8,
    pub flags:   u32,
    pub entries: Vec<IpmaEntry>
}

#[derive(Debug, Clone)]
pub struct IpmaEntry {
    pub item_id:      u32,
    pub associations: Vec<IpmaAssociation>
}

#[derive(Debug, Clone)]
pub struct IpmaAssociation {
    pub essential: bool, // If true, a decoder MUST understand this property to render the item.

    // This is a 1-BASED index into the `IpcoSection.properties` vector.
    // When resolving traits, remember to subtract 1 to get the actual array index!
    pub property_index: u16
}
#[derive(Debug, Clone)]
pub struct IdatSection {
    pub position: u64
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
    pub fn parse<T: ZByteReaderTrait>(data: &mut ZReader<T>) -> Result<Self, BmfErrors> {
        // skip one byte
        data.skip(1)?;
        // version = data[0] (must be 0)
        let flags = data.read_u8();
        let rows_minus_one = data.read_u8();
        let cols_minus_one = data.read_u8();
        let large = flags & 1 != 0;
        let (output_width, output_height) = if large {
            (data.get_u32_be_err()?, data.get_u32_be_err()?)
        } else {
            (data.get_u16_be_err()? as u32, data.get_u16_be_err()? as u32)
        };
        Ok(GridBox {
            columns: cols_minus_one as u32 + 1,
            rows: rows_minus_one as u32 + 1,
            output_width,
            output_height
        })
    }
}

#[derive(Debug, Clone)]
pub enum ColourInformation {
    Nclx {
        colour_primaries:         u16, // e.g., 1 = BT.709 (sRGB), 9 = BT.2020, 12 = Display P3
        transfer_characteristics: u16, // e.g., 1 = BT.709, 16 = PQ (HDR), 18 = HLG (HDR)
        matrix_coefficients:      u16, // e.g., 1 = BT.709, 6 = BT.601
        full_range_flag:          bool  // True = 0-255 (PC), False = 16-235 (TV/Studio)
    },
    IccProfile {
        profile_type: FourCC,  // 'rICC' or 'prof'
        profile_data: Vec<u8>  // The raw ICC blob to feed to a color management engine
    },
    Unknown {
        colour_type: FourCC,
        payload:     Vec<u8>
    }
}
