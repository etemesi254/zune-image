use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_core::log::trace;

use crate::bmf_reader::{BoxHeader, BoxSize, FourCC};
use crate::errors::HeicErrors;
use crate::header_structs::{
    ColourInformation, FtypHeader, HdlrSection, IdatSection, IinfSection, IlocExtent, IlocItem,
    IlocSection, InfeSection, IpcoSection, IpmaAssociation, IpmaEntry, IpmaSection, IprpSection,
    IrefEntry, IrefSection, ItemProperty, MetaSection, PitmSection,
};
use crate::utils::read_sized_int;

const MAX_SIZE: usize = 10 * 1024 * 1024;
const MAX_ENTRIES: u32 = 137;
#[inline]
fn subtract_value(value: usize, subtract: usize) -> Result<usize, HeicErrors> {
    match value.checked_sub(subtract) {
        None => Err(HeicErrors::WouldUnderflow {
            a: value,
            b: subtract,
        }),
        Some(e) => Ok(e)
    }
}
#[track_caller]
#[inline]
fn get_length(box_header: &BoxHeader) -> Result<usize, HeicErrors> {
    match box_header.total_size {
        BoxSize::Absolute(s) => Ok(s.saturating_sub(box_header.header_size) as usize),
        _ => Err(HeicErrors::ParseError {
            box_type: box_header.box_type,
            msg: "Needs absolute size".into(),
        })
    }
}
#[inline]
fn get_abs_length(box_header: &BoxHeader) -> Result<usize, HeicErrors> {
    match box_header.total_size {
        BoxSize::Absolute(s) => Ok(s as usize),
        _ => Err(HeicErrors::ParseError {
            box_type: box_header.box_type,
            msg: "Needs absolute size".into(),
        })
    }
}
pub fn decode_ftyp<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<FtypHeader, HeicErrors> {
    let mut full_size = get_length(box_header)?;

    if full_size < 8 {
        return Err(HeicErrors::PayloadTooShort {
            box_type: box_header.box_type,
            needed: 8,
            have: 8 - full_size,
        });
    }

    let major_brand = reader.read_fixed_bytes_or_error::<4>()?;
    let minor_version = reader.get_u32_be_err()?;

    full_size = full_size.saturating_sub(8);

    let mut compatible_brands = Vec::with_capacity(5);
    // > 3 means we can read 4 bytes from it, which is what we want as they make up
    // a full 4 byte compatible brand
    while full_size > 3 {
        let chunk = reader.read_fixed_bytes_or_error::<4>()?;
        compatible_brands.push(FourCC(chunk));

        full_size = full_size.saturating_sub(4);
    }
    // skip any size left
    reader.skip(full_size)?;

    Ok(FtypHeader {
        major_brand: FourCC(major_brand),
        minor_version,
        compatible_brands,
    })
}

///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │                 hdlr (Handler Reference Box)                │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x00   │ 4 bytes │ size              │ Total box size      │
///  │  0x04   │ 4 bytes │ type              │ 'hdlr'              │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x08   │ 1 byte  │ version           │ Usually 0x00        │
///  │  0x09   │ 3 bytes │ flags             │ Usually 0x000000    │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x0C   │ 4 bytes │ pre_defined       │ Always 0            │
///  │         │         │                   │                     │
///  │  0x10   │ 4 bytes │ handler_type      │ The Core Identity!  │
///  │         │         │                   │ (e.g., 'pict')      │
///  │         │         │                   │                     │
///  │  0x14   │ 4 bytes │ reserved[0]       │ Always 0            │
///  │  0x18   │ 4 bytes │ reserved[1]       │ Always 0            │
///  │  0x1C   │ 4 bytes │ reserved[2]       │ Always 0            │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x20   │ Variable│ name              │ Null-terminated     │
///  │         │         │                   │ UTF-8 String        │
///  │         │         │                   │                     │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
/// ```
pub fn decode_hdlr<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<HdlrSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;
    trace!("Decoding hdlr length: {bytes_left}");

    let version_and_flags = reader.get_u32_be_err()?;

    // This is a leftover from Apple's original QuickTime specification.
    // In old QuickTime files, this field was called component_type. In modern ISO files, it is strictly padding and must be set to 0.
    reader.skip(4)?;

    // Can be
    //  - pict: Picture data (This is what you will see in 99% of HEIC/HEIF files).
    //  - vide: Video data (Standard for MP4 video tracks).
    //  - soun: Audio data (Standard for MP4/M4A audio tracks).
    //  - meta: Timed metadata (Usually subtitles or timecodes).
    //  - null: Empty/filler track.
    let handler_type = reader.read_fixed_bytes_or_error::<4>()?;

    // QuickTime used to expect component_manufacturer, component_flags, and component_flags_mask here.
    // The ISO specification deprecated these, so it's just 12 bytes of zeros.
    reader.skip(12)?;

    bytes_left = subtract_value(bytes_left, 24)?;

    let mut name_bytes = Vec::new();

    while bytes_left > 0 {
        let b = reader.read_u8_err()?;
        bytes_left -= 1;
        if b == 0 {
            break;
        }
        name_bytes.push(b);
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }

    Ok(HdlrSection {
        version: (version_and_flags >> 24) as u8,
        flags: version_and_flags & 0x00FF_FFFF,
        handler_type: FourCC(handler_type),
        name: String::from_utf8_lossy(&name_bytes).into_owned(),
    })
}

///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │                 pitm (Primary Item Box)                     │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x00   │ 4 bytes │ size              │ Total box size      │
///  │  0x04   │ 4 bytes │ type              │ 'pitm'              │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x08   │ 1 byte  │ version           │ 0x00 or 0x01        │
///  │  0x09   │ 3 bytes │ flags             │ Usually 0x000000    │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x0C   │ 2 or 4  │ item_id           │ The ID of the main  │
///  │         │ bytes   │                   │ photo (e.g., #49)   │
///  │         │         │                   │                     │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
/// ```
///
pub fn decode_pitm<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<PitmSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;

    trace!("Decoding pitm length: {bytes_left}");

    let version_and_flags = reader.get_u32_be_err()?;
    let version = (version_and_flags >> 24) as u8;

    bytes_left = subtract_value(bytes_left, 4)?;

    let item_id = if version == 0 {
        let id = u32::from(reader.get_u16_be_err()?);
        bytes_left = subtract_value(bytes_left, 2)?;
        id
    } else {
        let id = reader.get_u32_be_err()?;
        bytes_left = subtract_value(bytes_left, 4)?;
        id
    };

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }

    Ok(PitmSection {
        version,
        flags: version_and_flags & 0x00FF_FFFF,
        item_id,
    })
}
///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │                 iinf (Item Information Box)                 │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x00   │ 4 bytes │ size              │ Total box size      │
///  │  0x04   │ 4 bytes │ type              │ 'iinf'              │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x08   │ 1 byte  │ version           │ 0x00, 0x01, or 0x02 │
///  │  0x09   │ 3 bytes │ flags             │ Usually 0x000000    │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x0C   │ 2 or 4  │ entry_count       │ Number of items in  │
///  │         │ bytes   │                   │ the file (e.g., 51) │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │Variable │Variable │ entries[]         │ A sequential list   │
///  │         │         │                   │ of 'infe' child     │
///  │         │         │                   │ boxes.              │
///  │         │         │                   │                     │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
/// ```
pub fn decode_iinf<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<IinfSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;

    trace!("Decoding iinf length: {bytes_left}");

    let version_and_flags = reader.get_u32_be_err()?;
    let version = (version_and_flags >> 24) as u8;

    bytes_left = subtract_value(bytes_left, 4)?;

    let entry_count = if version == 0 {
        let c = u32::from(reader.get_u16_be_err()?);
        bytes_left = subtract_value(bytes_left, 2)?;
        c
    } else {
        let c = reader.get_u32_be_err()?;
        bytes_left = subtract_value(bytes_left, 4)?;
        c
    };
    if entry_count > MAX_ENTRIES {
        // chosen by rng dice
        return Err(HeicErrors::Generic {
            msg: format!(
                "Entry count for IINF section too many {entry_count} (possibly corrupt) (max library supported entries {MAX_ENTRIES})"
            )
        });
    }

    let mut iinf = IinfSection {
        version,
        flags: version_and_flags & 0x00FF_FFFF,
        entry_count,
        entries: Vec::with_capacity(entry_count as usize),
    };

    // read infe sub blocks now
    while bytes_left >= 8 {
        let child_header = BoxHeader::read(reader)?;
        let child_size = get_abs_length(&child_header)?;

        if &child_header.box_type.0 == b"infe" {
            iinf.entries.push(decode_infe(reader, &child_header)?);
        } else {
            trace!("Skipping infe child header: {}", child_header.box_type);

            let payload_size = child_size.saturating_sub(child_header.header_size as usize);
            reader.skip(payload_size)?;
        }
        bytes_left = subtract_value(bytes_left, child_size)?;
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }
    Ok(iinf)
}

///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │                 iloc (Item Location Box)                    │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x00   │ 4 bytes │ size              │ Total box size      │
///  │  0x04   │ 4 bytes │ type              │ 'iloc'              │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x08   │ 1 byte  │ version           │ 0x00, 0x01, or 0x02 │
///  │  0x09   │ 3 bytes │ flags             │ Usually 0x000000    │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │ Packed into 1 byte: │
///  │  0x0C   │ 4 bits  │ offset_size       │  - Top 4 bits       │
///  │         │ 4 bits  │ length_size       │  - Bottom 4 bits    │
///  │         │         │                   │                     │
///  │         │         │                   │ Packed into 1 byte: │
///  │  0x0D   │ 4 bits  │ base_offset_size  │  - Top 4 bits       │
///  │         │ 4 bits  │ index_size        │  - Bottom 4 bits    │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x0E   │ 2 or 4  │ item_count        │ Number of items     │
///  │         │ bytes   │                   │ that have locations │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │Variable │Variable │ items[]           │ The actual map loop │
///  │         │         │                   │                     │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
/// ```
pub fn decode_iloc<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<IlocSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;

    let version_and_flags = reader.get_u32_be_err()?;
    let version = (version_and_flags >> 24) as u8;

    let b1 = reader.read_u8();
    let b2 = reader.read_u8();

    let offset_size = (b1 >> 4) & 0x0F;
    let length_size = b1 & 0x0F;

    let base_offset_size = (b2 >> 4) & 0x0F;
    let index_size = if version >= 1 { b2 & 0x0F } else { 0 };

    bytes_left = subtract_value(bytes_left, 6)?;

    let item_count = if version < 2 {
        let c = u32::from(reader.get_u16_be_err()?);
        bytes_left = subtract_value(bytes_left, 2)?;
        c
    } else {
        let c = reader.get_u32_be_err()?;
        bytes_left = subtract_value(bytes_left, 4)?;
        c
    };

    //  ┌─────────────────────────────────────────────────────────────┐
    //  │                     iloc Item Entry                         │
    //  ├─────────┬─────────┬───────────────────┬─────────────────────┤
    //  │ Offset  │ Size    │ Field Name        │ Description         │
    //  ├─────────┼─────────┼───────────────────┼─────────────────────┤
    //  │  +0x00  │ 2 or 4  │ item_id           │ e.g., 49            │
    //  │         │         │                   │                     │
    //  │  +0...  │ 2 bytes │ constr_method +   │ If Version >= 1     │
    //  │         │         │ reserved bits     │ (0=file, 1=idat)    │
    //  │         │         │                   │                     │
    //  │  +0...  │ 2 bytes │ data_ref_index    │ Usually 0           │
    //  │         │         │                   │                     │
    //  │  +0...  │ Variable│ base_offset       │ Size depends on     │
    //  │         │         │                   │ base_offset_size    │
    //  │         │         │                   │                     │
    //  │  +0...  │ 2 bytes │ extent_count      │ Number of chunks    │
    //  │         │         │                   │                     │
    //  ├─────────┼─────────┼───────────────────┼─────────────────────┤
    //  │         │         │ extents[]         │ Loop of extents     │
    //  │         │Variable │ (offset, length)  │ Size depends on     │
    //  │         │         │                   │ the nibbles above   │
    //  └─────────┴─────────┴───────────────────┴─────────────────────┘

    if item_count > MAX_ENTRIES {
        // chosen by rng dice
        return Err(HeicErrors::Generic {
            msg: format!("Entry count for ILOC section too many {item_count} (possibly corrupt) (max library supported entries {MAX_ENTRIES})")
        });
    }
    let mut items = Vec::with_capacity(item_count as usize);

    for _ in 0..item_count {
        let item_id = if version < 2 {
            let id = u32::from(reader.get_u16_be_err()?);
            bytes_left = subtract_value(bytes_left, 2)?;
            id
        } else {
            let id = reader.get_u32_be_err()?;
            bytes_left = subtract_value(bytes_left, 4)?;
            id
        };

        let construction_method = if version >= 1 {
            let cm = (reader.get_u16_be_err()? & 0x0F) as u8;
            bytes_left = subtract_value(bytes_left, 2)?;
            cm
        } else {
            0
        };

        let data_reference_index = reader.get_u16_be_err()?;
        let base_offset = read_sized_int(reader, base_offset_size)?;
        let extent_count = reader.get_u16_be_err()?;

        bytes_left = subtract_value(bytes_left, base_offset_size.saturating_add(4) as usize)?;

        let mut extents = Vec::with_capacity(extent_count as usize);

        for _ in 0..extent_count {
            let extent_index = if version >= 1 && index_size > 0 {
                let idx = read_sized_int(reader, index_size)?;

                bytes_left = subtract_value(bytes_left, index_size as usize)?;

                Some(idx)
            } else {
                None
            };

            let extent_offset = read_sized_int(reader, offset_size)?;
            let extent_length = read_sized_int(reader, length_size)?;

            // this can't overflow, because offset_size + length_size < 8, otherwise read_sized_int would panic
            let bytes_read = usize::from(offset_size + length_size);

            bytes_left = subtract_value(bytes_left, bytes_read)?;

            extents.push(IlocExtent {
                extent_index,
                extent_offset,
                extent_length,
            });
        }

        items.push(IlocItem {
            item_id,
            construction_method,
            data_reference_index,
            base_offset,
            extents,
        });
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }
    Ok(IlocSection {
        version,
        flags: version_and_flags & 0x00FF_FFFF,
        offset_size,
        length_size,
        base_offset_size,
        index_size,
        items,
    })
}

///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │                 iref (Item Reference Box)                   │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x00   │ 4 bytes │ size              │ Total box size      │
///  │  0x04   │ 4 bytes │ type              │ 'iref'              │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │  0x08   │ 1 byte  │ version           │ 0x00 or 0x01        │
///  │  0x09   │ 3 bytes │ flags             │ Usually 0x000000    │
///  │         │         │                   │                     │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │Variable │Variable │ references[]      │ A sequence of child │
///  │         │         │                   │ reference boxes     │
///  │         │         │                   │                     │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
/// ```
pub fn decode_iref<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<IrefSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;

    let version_and_flags = reader.get_u32_be_err()?;
    let version = (version_and_flags >> 24) as u8;

    bytes_left = subtract_value(bytes_left, 4)?;

    let mut references = Vec::new();

    while bytes_left >= 8 {
        //  ┌─────────────────────────────────────────────────────────────┐
        //  │               Child Reference Box (e.g., 'dimg')            │
        //  ├─────────┬─────────┬───────────────────┬─────────────────────┤
        //  │ Offset  │ Size    │ Field Name        │ Description         │
        //  ├─────────┼─────────┼───────────────────┼─────────────────────┤
        //  │  +0x00  │ 4 bytes │ size              │ Size of this ref    │
        //  │  +0x04  │ 4 bytes │ type              │ e.g., 'dimg', 'thmb'│
        //  │         │         │                   │                     │
        //  │  +0x08  │ 2 or 4  │ from_item_id      │ The Parent ID       │
        //  │         │ bytes   │                   │ (e.g., 49)          │
        //  │         │         │                   │                     │
        //  │  +...   │ 2 bytes │ reference_count   │ Number of children  │
        //  │         │         │                   │ (e.g., 48)          │
        //  │         │         │                   │                     │
        //  │  +...   │Variable │ to_item_ids[]     │ Array of Child IDs  │
        //  │         │         │                   │ (e.g., 1, 2, 3...)  │
        //  │         │         │                   │ Size matches 'from' │
        //  └─────────┴─────────┴───────────────────┴─────────────────────┘

        let child_header = BoxHeader::read(reader)?;

        let child_size = get_abs_length(&child_header)?;

        let mut child_bytes_left = child_size.saturating_sub(child_header.header_size as usize);

        let from_item_id = if version == 0 {
            let id = u32::from(reader.get_u16_be_err()?);
            child_bytes_left = subtract_value(child_bytes_left, 2)?;
            id
        } else {
            let id = reader.get_u32_be_err()?;
            child_bytes_left = subtract_value(child_bytes_left, 4)?;
            id
        };

        let ref_count = reader.get_u16_be_err()?;
        if ref_count as u32 > MAX_ENTRIES {
            // chosen by rng dice
            return Err(HeicErrors::Generic {
                msg: format!(
                    "Entry count for IREF section too many {ref_count} (possibly corrupt) (max library supported entries {MAX_ENTRIES})"
                )
            });
        }
        let mut to_item_ids = Vec::with_capacity(ref_count as usize);

        child_bytes_left = subtract_value(child_bytes_left, 2)?;

        for _ in 0..ref_count {
            let to_id = if version == 0 {
                let id = u32::from(reader.get_u16_be_err()?);
                child_bytes_left = subtract_value(child_bytes_left, 2)?;
                id
            } else {
                let id = reader.get_u32_be_err()?;
                child_bytes_left = subtract_value(child_bytes_left, 4)?;
                id
            };
            to_item_ids.push(to_id);
        }

        references.push(IrefEntry {
            reference_type: child_header.box_type,
            from_item_id,
            to_item_ids,
        });

        if child_bytes_left > 0 {
            reader.skip(child_bytes_left)?;
        }

        bytes_left = subtract_value(bytes_left, child_size)?;
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }
    Ok(IrefSection {
        version,
        flags: version_and_flags & 0x00FF_FFFF,
        references,
    })
}
///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │                 infe (Item Info Entry Box)                  │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │  0x00   │ 8 bytes │ size + type       │ e.g., 'infe'        │
///  │  0x08   │ 4 bytes │ version + flags   │ Usually v02         │
///  │  0x0C   │ 2 or 4  │ item_id           │ e.g., 49            │
///  │  0x0E/10│ 2 bytes │ item_protection   │ Usually 0           │
///  │  0x10/12│ 4 bytes │ item_type         │ e.g., 'grid'/'hvc1' │
///  │  0x14/16│ Variable│ item_name         │ Null-term string    │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
/// ```
pub fn decode_infe<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<InfeSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;

    let version_and_flags = reader.get_u32_be_err()?;
    let version = (version_and_flags >> 24) as u8;
    bytes_left = subtract_value(bytes_left, 4)?;

    let mut infe = InfeSection {
        version,
        flags: version_and_flags & 0x00FF_FFFF,
        item_id: 0,
        item_protection_index: 0,
        item_type: FourCC(*b"    "),
        item_name: String::new(),
        content_type: None,
        content_encoding: None,
        item_uri_type: None,
    };

    if version == 0 || version == 1 {
        infe.item_id = u32::from(reader.get_u16_be_err()?);
        infe.item_protection_index = reader.get_u16_be_err()?;
        bytes_left = bytes_left.saturating_sub(4);

        // HEIF rarely uses v0/v1, but we parse strings until null to be compliant
        let mut name_bytes = Vec::new();
        while bytes_left > 0 {
            let b = reader.read_u8();
            bytes_left -= 1;
            if b == 0 {
                break;
            }
            name_bytes.push(b);
        }
        infe.item_name = String::from_utf8_lossy(&name_bytes).into_owned();
      
    } else {
        infe.item_id = if version == 2 {
            let id = u32::from(reader.get_u16_be_err()?);
            bytes_left = subtract_value(bytes_left, 2)?;
            id
        } else {
            let id = reader.get_u32_be_err()?;
            bytes_left = subtract_value(bytes_left, 4)?;
            id
        };
        infe.item_protection_index = reader.get_u16_be_err()?;
        infe.item_type = FourCC(reader.read_fixed_bytes_or_error::<4>()?);

        bytes_left = subtract_value(bytes_left, 6)?;

        let mut name_bytes = Vec::new();

        while bytes_left > 0 {
            let b = reader.read_u8();
            bytes_left -= 1;
            if b == 0 {
                break;
            }
            name_bytes.push(b);
        }
        infe.item_name = String::from_utf8_lossy(&name_bytes).into_owned();
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }
    Ok(infe)
}

///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │                 iprp (Item Properties Box)                  │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │  0x00   │ 4 bytes │ size              │ Total box size      │
///  │  0x04   │ 4 bytes │ type              │ 'iprp'              │
///  │         │         │                   │                     │
///  │Variable │Variable │ child_boxes       │ Contains exactly 1  │
///  │         │         │                   │ 'ipco' and 1 'ipma' │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
///```
pub fn decode_iprp<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<IprpSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;

    trace!("Decoding IPRP section length {bytes_left:?}");

    let mut iprp = IprpSection::default();

    while bytes_left >= 8 {
        let child_header = BoxHeader::read(reader)?;

        let child_size = get_abs_length(&child_header)?;

        match &child_header.box_type.0 {
            b"ipco" => iprp.ipco = Some(decode_ipco(reader, &child_header)?),
            b"ipma" => iprp.ipma = Some(decode_ipma(reader, &child_header)?),
            _ => {
                trace!(
                    "Unsupported IPRP version {:?}",
                    child_header.box_type.as_str()
                );

                let payload_size = child_size.saturating_sub(child_header.header_size as usize);
                reader.skip(payload_size)?;
            }
        }
        bytes_left = subtract_value(bytes_left, child_size)?;
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }
    Ok(iprp)
}

///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │             ipco (Item Property Container Box)              │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │  0x00   │ 4 bytes │ size              │ Total box size      │
///  │  0x04   │ 4 bytes │ type              │ 'ipco'              │
///  │         │         │                   │                     │
///  │Variable │Variable │ properties[]      │ A sequence of trait │
///  │         │         │                   │ boxes (ispe, hvcC,  │
///  │         │         │                   │ colr, irot, etc.)   │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
/// ```
#[allow(clippy::too_many_lines)]
pub fn decode_ipco<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<IpcoSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;

    let mut ipco = IpcoSection::default();

    while bytes_left >= 8 {
        let child_header = BoxHeader::read(reader)?;
        let child_size = get_abs_length(&child_header)?;

        let payload_size = child_size.saturating_sub(child_header.header_size as usize);

        if payload_size > MAX_SIZE {
            return Err(HeicErrors::Generic {
                msg: format!(
                    "IPCO type {:?} with payload size {payload_size} exceeds max payload (max {MAX_SIZE} bytes)",
                    child_header.box_type,
                )
            });
        }
        match &child_header.box_type.0 {
            b"ispe" => {
                reader.skip(4)?; // skip version/flags

                let width = reader.get_u32_be_err()?;
                let height = reader.get_u32_be_err()?;

                ipco.properties.push(ItemProperty::Ispe { width, height });

                if payload_size > 12 {
                    reader.skip(payload_size - 12)?;
                }
            }
            b"irot" => {
                let angle = reader.read_u8() & 0x03;

                ipco.properties.push(ItemProperty::Irot {
                    angle_degrees: u16::from(angle) * 90
                });
                if payload_size > 1 {
                    reader.skip(payload_size - 1)?;
                }
            }
            b"colr" => {
                // Note: `colr` is NOT a FullBox. No version or flags.
                // It starts immediately with a 4-byte colour_type string.
                let colour_type_bytes = reader.read_fixed_bytes_or_error::<4>()?;
                let colour_type = FourCC(colour_type_bytes);

                let parsed_colr = match &colour_type.0 {
                    b"nclx" => {
                        let colour_primaries = reader.get_u16_be_err()?;
                        let transfer_characteristics = reader.get_u16_be_err()?;
                        let matrix_coefficients = reader.get_u16_be_err()?;

                        // The range flag is stored in the highest bit of the next byte.
                        // The remaining 7 bits are reserved and should be 0.
                        let flags = reader.read_u8();
                        let full_range_flag = (flags >> 7) != 0;

                        // Clean up any unexpected padding (nclx is exactly 11 bytes)
                        if payload_size > 11 {
                            reader.skip(payload_size - 11)?;
                        }

                        ColourInformation::Nclx {
                            colour_primaries,
                            transfer_characteristics,
                            matrix_coefficients,
                            full_range_flag,
                        }
                    }
                    b"rICC" | b"prof" => {
                        let icc_size = payload_size.saturating_sub(4);

                        let mut profile_data = vec![0; icc_size];
                        reader.read_exact_bytes(&mut profile_data)?;

                        ColourInformation::IccProfile {
                            profile_type: colour_type,
                            profile_data,
                        }
                    }
                    _ => {
                        // Fallback for unknown color types
                        let skip_size = payload_size.saturating_sub(4);

                        let mut payload = vec![0; skip_size];
                        reader.read_exact_bytes(&mut payload)?;

                        ColourInformation::Unknown {
                            colour_type,
                            payload,
                        }
                    }
                };

                ipco.properties.push(ItemProperty::Colr(parsed_colr));
            }
            b"hvcC" | b"av1C" => {
                // For complex codecs, read raw bytes so decoders can use them later
                let mut payload = vec![0; payload_size];
                reader.read_exact_bytes(&mut payload)?;

                let prop = match &child_header.box_type.0 {
                    b"hvcC" => ItemProperty::HvcC { payload },
                    b"av1C" => ItemProperty::Av1C { payload },
                    _ => unreachable!()
                };
                ipco.properties.push(prop);
            }
            b"pixi" => {
                // skip bits and versions
                reader.skip(4)?;
                let channels = reader.read_u8();

                if channels > 4 {
                    return Err(
                        HeicErrors::Generic {
                            msg: format!("Too many image channels {channels} exceeding 4")
                        }
                    );
                }
                let mut bits_per_channel = Vec::with_capacity(channels as usize);
                for _ in 0..channels {
                    bits_per_channel.push(reader.read_u8());
                }

                ipco.properties.push(ItemProperty::Pixi {
                    channels,
                    bits_per_channel,
                });

            }
            b"auxC" => {
                reader.skip(4)?;

                // Read null-terminated string for the aux_type
                let mut type_bytes = Vec::new();
                let mut bytes_read = 4; // We already read the 4-byte header

                loop {
                    let b = reader.read_u8();
                    bytes_read += 1;
                    if b == 0 {
                        break;
                    }
                    if type_bytes.len() > MAX_SIZE {
                        return Err(HeicErrors::Generic {
                            msg: format!(
                                "Null terminated value with len {} exceeds max size ({MAX_SIZE} bytes))",
                                type_bytes.len()
                            )
                        });
                    }
                    type_bytes.push(b);
                }

                let aux_type = String::from_utf8_lossy(&type_bytes).into_owned();

                // Read the remaining payload as the subtype (usually empty for Alpha)
                let subtype_size = payload_size.saturating_sub(bytes_read);

                let mut subtype = vec![0; subtype_size];
                reader.read_exact_bytes(&mut subtype)?;

                ipco.properties
                    .push(ItemProperty::AuxC { aux_type, subtype });
            }
            _ => {
                // Unknown properties (like auxC, pixi) are swept into the generic bin to save space
                let mut payload = vec![0; payload_size];
                reader.read_exact_bytes(&mut payload)?;

                ipco.properties.push(ItemProperty::Unknown {
                    box_type: child_header.box_type,
                    payload,
                });
            }
        }
        bytes_left = subtract_value(bytes_left, child_size)?;
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }
    Ok(ipco)
}
///```text
///  ┌─────────────────────────────────────────────────────────────┐
///  │             ipma (Item Property Association Box)            │
///  ├─────────┬─────────┬───────────────────┬─────────────────────┤
///  │ Offset  │ Size    │ Field Name        │ Description         │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │  0x00   │ 4 bytes │ size              │ Total box size      │
///  │  0x04   │ 4 bytes │ type              │ 'ipma'              │
///  │  0x08   │ 1 byte  │ version           │ 0x00 or 0x01        │
///  │  0x09   │ 3 bytes │ flags             │ See 'Gotchas' below!│
///  │         │         │                   │                     │
///  │  0x0C   │ 4 bytes │ entry_count       │ Number of items     │
///  │         │         │                   │ getting properties  │
///  ├─────────┼─────────┼───────────────────┼─────────────────────┤
///  │         │         │                   │                     │
///  │Variable │Variable │ entries[]         │ The mapping loop    │
///  │         │         │                   │                     │
///  └─────────┴─────────┴───────────────────┴─────────────────────┘
/// ```
pub fn decode_ipma<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<IpmaSection, HeicErrors> {
    let mut bytes_left = get_length(box_header)?;

    let version_and_flags = reader.get_u32_be_err()?;
    let entry_count = reader.get_u32_be_err()?;

    bytes_left = subtract_value(bytes_left, 8)?;

    let version = (version_and_flags >> 24) as u8;
    let flags = version_and_flags & 0x00FF_FFFF;
    if entry_count > MAX_ENTRIES {
        // chosen by rng dice
        return Err(HeicErrors::Generic {
            msg: format!(
                "Entry count for IPMA section too many {entry_count} (possibly corrupt) (max library supported entries {MAX_ENTRIES})"
            )
        });
    }

    let mut entries = Vec::with_capacity(entry_count as usize);

    for _ in 0..entry_count {
        let item_id = if version >= 1 {
            let id = reader.get_u32_be_err()?;
            bytes_left = subtract_value(bytes_left, 4)?;
            id
        } else {
            let id = u32::from(reader.get_u16_be_err()?);
            bytes_left = subtract_value(bytes_left, 2)?;
            id
        };

        let assoc_count = reader.read_u8();

        bytes_left = subtract_value(bytes_left, 1)?;

        if entry_count > MAX_ENTRIES {
            // chosen by rng dice
            return Err(HeicErrors::Generic {
                msg: format!(
                    "Entry count for IPMA sub-section too many {entry_count} (possibly corrupt) (max library supported entries {MAX_ENTRIES})"
                )
            });
        }

        let mut associations = Vec::with_capacity(assoc_count as usize);

        for _ in 0..assoc_count {
            if (flags & 1) != 0 {
                let val = reader.get_u16_be_err()?;
                bytes_left = subtract_value(bytes_left, 2)?;
                associations.push(IpmaAssociation {
                    essential: (val >> 15) != 0,
                    property_index: val & 0x7FFF,
                });
            } else {
                let val = reader.read_u8();
                bytes_left = subtract_value(bytes_left, 1)?;
                associations.push(IpmaAssociation {
                    essential: (val >> 7) != 0,
                    property_index: u16::from(val & 0x7F),
                });
            }
        }
        entries.push(IpmaEntry {
            item_id,
            associations,
        });
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }
    Ok(IpmaSection {
        version,
        flags,
        entries,
    })
}

#[track_caller]
pub fn decode_meta<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, box_header: &BoxHeader,
) -> Result<MetaSection, HeicErrors> {
    if &box_header.box_type.0 != b"meta" {
        return Err(HeicErrors::ParseError {
            box_type: box_header.box_type,
            msg: "Expected META".into(),
        });
    }

    let mut bytes_left = get_length(box_header)?;

    if bytes_left < 4 {
        return Err(HeicErrors::PayloadTooShort {
            box_type: box_header.box_type,
            needed: 4,
            have: bytes_left,
        });
    }

    reader.skip(4)?;

    bytes_left -= 4;

    let mut meta = MetaSection {
        ..Default::default()
    };

    while bytes_left >= 8 {
        let child_header = BoxHeader::read(reader)?;
        let child_size = get_abs_length(&child_header)?;

        if child_size > bytes_left {
            return Err(HeicErrors::ParseError {
                box_type: child_header.box_type,
                msg: "Child box exceeds meta bounds".into(),
            });
        }

        match &child_header.box_type.0 {
            b"hdlr" => meta.hdlr = Some(decode_hdlr(reader, &child_header)?),
            b"pitm" => meta.pitm = Some(decode_pitm(reader, &child_header)?),
            b"iinf" => meta.iinf = Some(decode_iinf(reader, &child_header)?),
            b"iloc" => meta.iloc = Some(decode_iloc(reader, &child_header)?),
            b"iprp" => meta.iprp = Some(decode_iprp(reader, &child_header)?),
            b"iref" => meta.iref = Some(decode_iref(reader, &child_header)?),
            b"idat" => {
                let pos = reader.position()?;
                meta.idat = Some(IdatSection { position: pos });
                let size = get_length(&child_header)?;
                reader.skip(size)?;
            }
            _ => {
                trace!(
                    "Unknown meta child type: {:?} skipping",
                    child_header.box_type.as_str()
                );

                let payload_size = child_size.saturating_sub(child_header.header_size as usize);
                reader.skip(payload_size)?;
            }
        }
        bytes_left -= child_size;
    }

    if bytes_left > 0 {
        reader.skip(bytes_left)?;
    }
    Ok(meta)
}
#[cfg(test)]
mod tests {
    use zune_core::bytestream::ZCursor;

    use super::*;

    /// Build a minimal valid ISOBMFF box: 4-byte size + 4-byte type + payload
    fn make_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = (8 + payload.len()) as u32;
        let mut b = Vec::new();
        b.extend_from_slice(&size.to_be_bytes());
        b.extend_from_slice(fourcc);
        b.extend_from_slice(payload);
        b
    }

    #[test]
    fn test_parse_single_ftyp_box() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"isom"); // major brand
        payload.extend_from_slice(&0u32.to_be_bytes()); // minor version
        payload.extend_from_slice(b"isom");
        payload.extend_from_slice(b"iso2");
        payload.extend_from_slice(b"mp41");
        payload.extend_from_slice(b"mp42");
        payload.extend_from_slice(b"mp44");

        let raw = make_box(b"ftyp", &payload);
        let cursor = ZCursor::new(raw);
        let mut reader = ZReader::new(cursor);
        let response = BoxHeader::read(&mut reader).unwrap();
        let value = decode_ftyp(&mut reader, &response).unwrap();

        assert_eq!(value.major_brand, FourCC(*b"isom"));
        assert_eq!(value.minor_version, 0);
        assert_eq!(value.compatible_brands.len(), 5);
    }
}
