//! `unci` items: uncompressed images (ISO/IEC 23001-17).
//!
//! The samples are stored as they are, component by component (`cmpd`
//! names them: R, G, B, Y, Cb, Cr, alpha, ...), laid out as `uncC` says
//! (bit depth, interleaving, padding and alignment, tiles), optionally
//! compressed (`cmpC`: zlib, deflate or Brotli; `icef`: where each
//! compressed unit is).
//!
//! The decoding follows libheif 1.21 (`codecs/uncompressed`), including how
//! it sizes tiles, and its conversion to 8-bit output, so the output matches
//! libheif's.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use crate::colour::Conversion;
use crate::errors::HeicErrors;
use crate::header_structs::ItemProperty;

fn error(msg: impl Into<String>) -> HeicErrors {
    HeicErrors::Generic { msg: msg.into() }
}

// component types (`cmpd`)
const MONOCHROME: u16 = 0;
const Y: u16 = 1;
const CB: u16 = 2;
const CR: u16 = 3;
const RED: u16 = 4;
const GREEN: u16 = 5;
const BLUE: u16 = 6;
const ALPHA: u16 = 7;
const PADDED: u16 = 12;

// interleave types (`uncC`)
const INTERLEAVE_COMPONENT: u8 = 0;
const INTERLEAVE_PIXEL: u8 = 1;
const INTERLEAVE_MIXED: u8 = 2;
const INTERLEAVE_ROW: u8 = 3;
const INTERLEAVE_TILE_COMPONENT: u8 = 4;

// sampling types (`uncC`)
const SAMPLING_NONE: u8 = 0;
const SAMPLING_422: u8 = 1;
const SAMPLING_420: u8 = 2;

// compressed unit types (`cmpC`)
const UNIT_TILE: u8 = 2;

/// The output planes; `Plane::channel` indexes them.
const NUM_CHANNELS: usize = 7;

/// Where a component's samples go.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Channel {
    /// Luma, or the grey of a monochrome image
    Y = 0,
    Cb = 1,
    Cr = 2,
    R = 3,
    G = 4,
    B = 5,
    A = 6,
}

/// What the components make up.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum UnciColour {
    /// Grey, possibly with alpha
    Mono,
    /// R, G and B, possibly with alpha
    Rgb,
    /// Y, Cb and Cr
    YCbCr,
}

/// One component of `uncC`, with its type from `cmpd`.
#[derive(Copy, Clone, Debug)]
#[allow(clippy::struct_field_names)]
struct Component {
    component_type: u16,
    bit_depth: u32,
    format: u8,
    align_size: u32,
}

/// The description of an `unci` item: its `uncC`, `cmpd`, `cmpC` and `icef`
/// properties.
#[derive(Clone, Debug)]
pub(crate) struct UnciHeader {
    components: Vec<Component>,
    sampling: u8,
    interleave: u8,
    block_size: u8,
    /// components little endian, block pad lsb, block little endian, block
    /// reversed, pad unknown (bits 7 to 3)
    flags: u8,
    pixel_size: u32,
    row_align: u32,
    tile_align: u32,
    tile_cols: u32,
    tile_rows: u32,
    /// `cmpC`: compression type and compressed unit type
    compression: Option<([u8; 4], u8)>,
    /// `icef`: (offset, size) of each compressed unit in the item's data
    units: Option<Vec<(u64, u64)>>,
}

/// Big-endian reader over a property's payload.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    name: &'static str,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], name: &'static str) -> Self {
        Self { data, pos: 0, name }
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], HeicErrors> {
        let bytes = self
            .pos
            .checked_add(n)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or_else(|| error(format!("'{}' box is too short", self.name)))?;
        self.pos += n;
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8, HeicErrors> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, HeicErrors> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, HeicErrors> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn fourcc(&mut self) -> Result<[u8; 4], HeicErrors> {
        let b = self.bytes(4)?;
        Ok([b[0], b[1], b[2], b[3]])
    }
    /// An unsigned big-endian integer of `bits` (a multiple of 8, up to 64)
    fn uint(&mut self, bits: u32) -> Result<u64, HeicErrors> {
        let b = self.bytes((bits / 8) as usize)?;
        Ok(b.iter().fold(0, |v, &x| (v << 8) | u64::from(x)))
    }
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
}

/// `x` rounded up to a multiple of `alignment` (unchanged if 0)
fn align_up(x: u64, alignment: u32) -> u64 {
    let alignment = u64::from(alignment);
    if alignment == 0 || x.is_multiple_of(alignment) {
        x
    } else {
        x + (alignment - x % alignment)
    }
}

impl UnciHeader {
    /// Parse the `unci` properties of an item (its associated properties).
    pub(crate) fn parse<'p>(
        properties: impl Iterator<Item = &'p ItemProperty>,
    ) -> Result<Self, HeicErrors> {
        let (mut cmpd, mut unc_c, mut cmp_c, mut icef) = (None, None, None, None);
        for property in properties {
            if let ItemProperty::Unknown { box_type, payload } = property {
                match &box_type.0 {
                    b"cmpd" => cmpd = Some(payload.as_slice()),
                    b"uncC" => unc_c = Some(payload.as_slice()),
                    b"cmpC" => cmp_c = Some(payload.as_slice()),
                    b"icef" => icef = Some(payload.as_slice()),
                    _ => {}
                }
            }
        }
        let unc_c = unc_c.ok_or_else(|| error("unci item without uncC box"))?;

        // cmpd: the type of each component
        let mut component_types = Vec::new();
        if let Some(cmpd) = cmpd {
            let mut r = Reader::new(cmpd, "cmpd");
            let count = r.u32()?;
            for _ in 0..count {
                let component_type = r.u16()?;
                if component_type >= 0x8000 {
                    // a URI naming the type, null terminated
                    while r.u8()? != 0 {}
                }
                component_types.push(component_type);
            }
        }

        let mut r = Reader::new(unc_c, "uncC");
        let version = r.u8()?;
        r.bytes(3)?; // flags
        let profile = r.fourcc()?;
        let mut header = UnciHeader {
            components: Vec::new(),
            sampling: SAMPLING_NONE,
            interleave: INTERLEAVE_PIXEL,
            block_size: 0,
            flags: 0,
            pixel_size: 0,
            row_align: 0,
            tile_align: 0,
            tile_cols: 1,
            tile_rows: 1,
            compression: None,
            units: None,
        };
        match version {
            // a profile stands for 8-bit pixel-interleaved components
            1 => {
                let types: &[u16] = match &profile {
                    b"rgb3" => &[RED, GREEN, BLUE],
                    b"rgba" => &[RED, GREEN, BLUE, ALPHA],
                    b"abgr" => &[ALPHA, BLUE, GREEN, RED],
                    _ => return Err(error("uncC version 1 with an unknown profile")),
                };
                header.components = types
                    .iter()
                    .map(|&component_type| Component {
                        component_type,
                        bit_depth: 8,
                        format: 0,
                        align_size: 0,
                    })
                    .collect();
            }
            0 => {
                if cmpd.is_none() {
                    return Err(error("unci item without cmpd box"));
                }
                let count = r.u32()?;
                if count as usize > r.remaining() / 5 {
                    return Err(error("uncC box is too short for its components"));
                }
                for _ in 0..count {
                    let index = usize::from(r.u16()?);
                    let bit_depth = u32::from(r.u8()?) + 1;
                    let format = r.u8()?;
                    let align_size = u32::from(r.u8()?);
                    let component_type = *component_types
                        .get(index)
                        .ok_or_else(|| error("Invalid component index in uncC box"))?;
                    header.components.push(Component {
                        component_type,
                        bit_depth,
                        format,
                        align_size,
                    });
                }
                header.sampling = r.u8()?;
                header.interleave = r.u8()?;
                header.block_size = r.u8()?;
                header.flags = r.u8()?;
                header.pixel_size = r.u32()?;
                header.row_align = r.u32()?;
                header.tile_align = r.u32()?;
                header.tile_cols = r.u32()?.saturating_add(1);
                header.tile_rows = r.u32()?.saturating_add(1);
            }
            v => return Err(error(format!("uncC version {v} is not supported"))),
        }

        if let Some(cmp_c) = cmp_c {
            let mut r = Reader::new(cmp_c, "cmpC");
            if r.u8()? != 0 {
                return Err(error("cmpC version is not supported"));
            }
            r.bytes(3)?;
            let compression_type = r.fourcc()?;
            let unit_type = r.u8()?;
            if unit_type > 4 {
                return Err(error("Unsupported cmpC compressed unit type"));
            }
            header.compression = Some((compression_type, unit_type));
        }

        if let Some(icef) = icef {
            const OFFSET_BITS: [u32; 5] = [0, 16, 24, 32, 64];
            const SIZE_BITS: [u32; 5] = [8, 16, 24, 32, 64];
            let mut r = Reader::new(icef, "icef");
            if r.u8()? != 0 {
                return Err(error("icef version is not supported"));
            }
            r.bytes(3)?;
            let codes = r.u8()?;
            let count = r.u32()? as usize;
            let offset_bits = *OFFSET_BITS
                .get(usize::from(codes >> 5))
                .ok_or_else(|| error("Unsupported icef unit offset code"))?;
            let size_bits = *SIZE_BITS
                .get(usize::from((codes >> 2) & 7))
                .ok_or_else(|| error("Unsupported icef unit size code"))?;
            let entry_size = ((offset_bits + size_bits) / 8) as usize;
            if count > r.remaining() / entry_size {
                return Err(error("icef box is too short for its units"));
            }
            let mut units = Vec::with_capacity(count);
            // offset code 0: the units follow each other from the start
            let mut implied_offset = 0u64;
            for _ in 0..count {
                let offset = if offset_bits == 0 { implied_offset } else { r.uint(offset_bits)? };
                let size = r.uint(size_bits)?;
                implied_offset = implied_offset
                    .checked_add(size)
                    .ok_or_else(|| error("icef unit offsets overflow"))?;
                units.push((offset, size));
            }
            header.units = Some(units);
        }
        Ok(header)
    }

    /// What the components make up, and whether there is alpha; an error
    /// for images libheif does not decode either.
    pub(crate) fn colour(&self) -> Result<(UnciColour, bool), HeicErrors> {
        let mut set = 0u32;
        for c in &self.components {
            if c.component_type > ALPHA && c.component_type != PADDED {
                return Err(error(format!(
                    "Uncompressed image with component type {} is not supported",
                    c.component_type
                )));
            }
            if c.component_type != PADDED {
                set |= 1 << c.component_type;
            }
        }
        let has_alpha = set & (1 << ALPHA) != 0;
        let colour = match set & !(1 << ALPHA) {
            s if s == (1 << RED) | (1 << GREEN) | (1 << BLUE) => UnciColour::Rgb,
            s if s == (1 << MONOCHROME) => UnciColour::Mono,
            s if s == (1 << Y) | (1 << CB) | (1 << CR) && !has_alpha => UnciColour::YCbCr,
            _ => {
                return Err(error(
                    "Uncompressed image with an unsupported set of components",
                ));
            }
        };
        Ok((colour, has_alpha))
    }

    /// Chroma subsampling (horizontal, vertical)
    fn subsampling(&self) -> (u32, u32) {
        match self.sampling {
            SAMPLING_422 => (2, 1),
            SAMPLING_420 => (2, 2),
            _ => (1, 1),
        }
    }

    /// The checks of libheif's `uncompressed_image_type_is_supported` and
    /// `check_header_validity`, plus a few for layouts that do not make
    /// sense (and that libheif mishandles).
    fn check(&self, width: u32, height: u32) -> Result<(), HeicErrors> {
        self.colour()?;
        for c in &self.components {
            if c.bit_depth > 16 {
                return Err(error(format!(
                    "Uncompressed image with {}-bit components is not supported",
                    c.bit_depth
                )));
            }
            if c.format != 0 {
                return Err(error(
                    "Uncompressed image with non-integer components is not supported",
                ));
            }
            if c.align_size > 2 {
                return Err(error(format!(
                    "Uncompressed image with component_align_size {} is not supported",
                    c.align_size
                )));
            }
            if c.align_size != 0 && c.bit_depth > c.align_size * 8 {
                return Err(error(
                    "Uncompressed component larger than its component_align_size",
                ));
            }
        }
        match self.sampling {
            SAMPLING_NONE => {
                if self.interleave == INTERLEAVE_MIXED {
                    return Err(error(
                        "Mixed interleave is only valid with chroma subsampling",
                    ));
                }
            }
            SAMPLING_422 | SAMPLING_420 => {
                if self.interleave != INTERLEAVE_COMPONENT && self.interleave != INTERLEAVE_MIXED {
                    return Err(error(
                        "YCbCr subsampling is only valid with component or mixed interleave",
                    ));
                }
                if self.interleave == INTERLEAVE_COMPONENT && !self.row_align.is_multiple_of(2) {
                    return Err(error("YCbCr subsampling needs an even row_align_size"));
                }
                let needed = if self.sampling == SAMPLING_420 { 4 } else { 2 };
                if !self.tile_align.is_multiple_of(needed) {
                    return Err(error("YCbCr subsampling with an invalid tile_align_size"));
                }
            }
            s => {
                return Err(error(format!(
                    "Uncompressed sampling type {s} is not supported"
                )));
            }
        }
        if self.interleave > INTERLEAVE_TILE_COMPONENT {
            return Err(error(format!(
                "Uncompressed interleave type {} is not supported",
                self.interleave
            )));
        }
        if self.block_size != 0 {
            return Err(error("Uncompressed image with blocks is not supported"));
        }
        if self.flags & 0x80 != 0 && self.components.iter().any(|c| c.bit_depth != 8) {
            return Err(error(
                "Little-endian uncompressed components are not supported",
            ));
        }
        if self.flags & 0x70 != 0 {
            return Err(error("Uncompressed block flags are not supported"));
        }
        if self.pixel_size != 0 && self.interleave != INTERLEAVE_PIXEL {
            return Err(error("pixel_size is only valid with pixel interleave"));
        }
        if self.tile_cols > width || self.tile_rows > height {
            return Err(error("More tiles than pixels in uncC box"));
        }
        if !width.is_multiple_of(self.tile_cols) || !height.is_multiple_of(self.tile_rows) {
            return Err(error(
                "Invalid tile size (image size not a multiple of the tile size)",
            ));
        }
        let (sub_x, sub_y) = self.subsampling();
        if !(width / self.tile_cols).is_multiple_of(sub_x)
            || !(height / self.tile_rows).is_multiple_of(sub_y)
        {
            return Err(error(
                "Subsampled tile size is not a multiple of the subsampling",
            ));
        }
        Ok(())
    }

    /// Decode the item, `data` being its bytes (all extents), into planes.
    /// `max_bytes` bounds the decompressed size.
    fn decode_planes(
        &self, data: &[u8], width: u32, height: u32, max_bytes: usize,
    ) -> Result<Planes, HeicErrors> {
        self.check(width, height)?;
        let (sub_x, sub_y) = self.subsampling();
        let tile_w = width / self.tile_cols;
        let tile_h = height / self.tile_rows;

        // decompress first, and check that there is enough data for the
        // samples (padding only adds to it) before allocating the planes
        let source = Source::new(self, data, max_bytes)?;
        let (sx, sy) = (u64::from(sub_x), u64::from(sub_y));
        let min_bits: u64 = self
            .components
            .iter()
            .map(|c| {
                let pixels = if matches!(c.component_type, CB | CR) {
                    (u64::from(width) / sx) * (u64::from(height) / sy)
                } else {
                    u64::from(width) * u64::from(height)
                };
                u64::from(c.bit_depth) * pixels
            })
            .sum();
        if let Some(len) = source.whole_len()
            && (len as u64) < min_bits / 8
        {
            return Err(error("Uncompressed image data is too short"));
        }

        let mut planes = Planes::default();
        let mut entries = Vec::with_capacity(self.components.len());
        for c in &self.components {
            let channel = match c.component_type {
                MONOCHROME | Y => Some(Channel::Y),
                CB => Some(Channel::Cb),
                CR => Some(Channel::Cr),
                RED => Some(Channel::R),
                GREEN => Some(Channel::G),
                BLUE => Some(Channel::B),
                ALPHA => Some(Channel::A),
                _ => None,
            };
            let chroma = matches!(channel, Some(Channel::Cb | Channel::Cr));
            let (sx, sy) = if chroma { (sub_x, sub_y) } else { (1, 1) };
            if let Some(channel) = channel {
                let slot = &mut planes.planes[channel as usize];
                if slot.is_some() {
                    return Err(error("Uncompressed image with the same component twice"));
                }
                let (w, h) = ((width / sx) as usize, (height / sy) as usize);
                let size = w.checked_mul(h).ok_or_else(|| error("Image too large"))?;
                *slot = Some(Plane {
                    width: w,
                    bits: c.bit_depth,
                    data: vec![0; size],
                });
            }
            let bytes = c.bit_depth.div_ceil(8);
            entries.push(Entry {
                channel,
                bits: c.bit_depth,
                align: c.align_size,
                bytes,
                tile_w: tile_w / sx,
                tile_h: tile_h / sy,
                sub: (sx, sy),
            });
        }

        let mut decoder = TileDecoder {
            header: self,
            entries: &entries,
            planes: &mut planes,
            tile_w,
            tile_h,
            num_tiles: u64::from(self.tile_cols) * u64::from(self.tile_rows),
        };
        for tile_y in 0..self.tile_rows {
            for tile_x in 0..self.tile_cols {
                let tile_index = u64::from(tile_x) + u64::from(tile_y) * u64::from(self.tile_cols);
                decoder.decode_tile(&source, tile_index, tile_x * tile_w, tile_y * tile_h)?;
            }
        }
        Ok(planes)
    }

    /// Decode the item into `output`: `channels` interleaved 8-bit samples
    /// per pixel (1: grey/luma, 2: grey + alpha, 3: RGB, 4: RGBA), as
    /// libheif converts it. `conversion` is used for YCbCr images.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn decode(
        &self, data: &[u8], width: usize, height: usize, channels: usize,
        conversion: Option<&Conversion>, max_bytes: usize, output: &mut [u8],
    ) -> Result<(), HeicErrors> {
        let (width_u32, height_u32) = (
            u32::try_from(width).map_err(|_| error("Image too large"))?,
            u32::try_from(height).map_err(|_| error("Image too large"))?,
        );
        let (colour, _) = self.colour()?;
        let planes = self.decode_planes(data, width_u32, height_u32, max_bytes)?;
        let output = output
            .get_mut(..width * height * channels)
            .ok_or_else(|| error("Output buffer too small"))?;
        let plane = |c: Channel| planes.planes[c as usize].as_ref().map(Plane::to_8bit);
        let opaque = vec![255u8; width * height];

        match (colour, channels) {
            (UnciColour::Mono | UnciColour::YCbCr, 1) => {
                output.copy_from_slice(&plane(Channel::Y).unwrap_or_default());
            }
            (UnciColour::Mono, 2..=4) => {
                let grey = plane(Channel::Y).unwrap_or_default();
                let alpha = plane(Channel::A).unwrap_or(opaque);
                for ((px, &g), &a) in output.chunks_exact_mut(channels).zip(&grey).zip(&alpha) {
                    px.fill(g);
                    if channels != 3 {
                        px[channels - 1] = a;
                    }
                }
            }
            (UnciColour::Rgb, 3 | 4) => {
                let (r, g, b) = (
                    plane(Channel::R).unwrap_or_default(),
                    plane(Channel::G).unwrap_or_default(),
                    plane(Channel::B).unwrap_or_default(),
                );
                let a = plane(Channel::A).unwrap_or(opaque);
                for (i, px) in output.chunks_exact_mut(channels).enumerate() {
                    px[0] = r[i];
                    px[1] = g[i];
                    px[2] = b[i];
                    if channels == 4 {
                        px[3] = a[i];
                    }
                }
            }
            (UnciColour::YCbCr, 3 | 4) => {
                let conversion =
                    conversion.ok_or_else(|| error("No colour conversion for a YCbCr image"))?;
                let (sub_x, sub_y) = self.subsampling();
                let (sub_x, sub_y) = (sub_x as usize, sub_y as usize);
                let (y, cb, cr) = (
                    plane(Channel::Y).unwrap_or_default(),
                    plane(Channel::Cb).unwrap_or_default(),
                    plane(Channel::Cr).unwrap_or_default(),
                );
                let chroma_w = width / sub_x;
                for (row, dst) in output.chunks_exact_mut(width * channels).enumerate() {
                    let c = (row / sub_y) * chroma_w;
                    conversion.convert_row(
                        &y[row * width..][..width],
                        &cb[c..][..chroma_w],
                        &cr[c..][..chroma_w],
                        sub_x,
                        dst,
                        channels,
                    );
                }
            }
            _ => {
                return Err(error(format!(
                    "Cannot convert this uncompressed image to {channels} channels"
                )));
            }
        }
        Ok(())
    }
}

/// One decoded component plane, `bits` per sample.
struct Plane {
    width: usize,
    bits: u32,
    data: Vec<u16>,
}

impl Plane {
    /// The samples reduced (or expanded) to 8 bits, as libheif does:
    /// more than 8 bits are shifted down (no rounding), fewer are scaled up
    /// by repeating their bits.
    fn to_8bit(&self) -> Vec<u8> {
        match self.bits {
            8 => self.data.iter().map(|&v| v as u8).collect(),
            b if b > 8 => self.data.iter().map(|&v| (v >> (b - 8)) as u8).collect(),
            b => {
                let mut bit = 1u32 << (16 - b);
                let mut factor = bit;
                loop {
                    bit >>= b;
                    if bit == 0 {
                        break;
                    }
                    factor |= bit;
                }
                self.data
                    .iter()
                    .map(|&v| ((u32::from(v) * factor) >> 8) as u8)
                    .collect()
            }
        }
    }
}

#[derive(Default)]
struct Planes {
    planes: [Option<Plane>; NUM_CHANNELS],
}

/// A component as the tile decoders see it (libheif's `ChannelListEntry`).
#[derive(Copy, Clone)]
struct Entry {
    /// `None` for padding
    channel: Option<Channel>,
    bits: u32,
    /// component_align_size, in bytes
    align: u32,
    /// bytes of one sample in libheif's planes
    bytes: u32,
    /// size of the component in one tile (chroma is subsampled)
    tile_w: u32,
    tile_h: u32,
    /// subsampling of the component
    sub: (u32, u32),
}

impl Entry {
    /// Bits of one sample, padded to `align` bytes if it has one
    fn padded_bits(&self) -> u32 {
        if self.align > 0 {
            align_up(u64::from(self.bytes), self.align) as u32 * 8
        } else {
            self.bits
        }
    }
    /// Bytes of one tile row in libheif's planes
    fn plane_row_bytes(&self) -> u64 {
        u64::from(self.tile_w) * u64::from(self.bytes)
    }
}

/// The item's data, decompressed if it is compressed.
enum Source<'a> {
    /// Uncompressed
    Raw(&'a [u8]),
    /// Decompressed as a whole (one unit, or all `icef` units one after the
    /// other)
    Whole(Vec<u8>),
    /// One compressed unit per tile (`icef`), decompressed when needed
    PerTile {
        data: &'a [u8],
        units: &'a [(u64, u64)],
        compression: [u8; 4],
        max_bytes: usize,
    },
}

impl<'a> Source<'a> {
    fn new(header: &'a UnciHeader, data: &'a [u8], max_bytes: usize) -> Result<Self, HeicErrors> {
        let Some((compression, unit_type)) = header.compression else {
            return Ok(Source::Raw(data));
        };
        let unit = |&(offset, size): &(u64, u64)| {
            slice(data, offset, size).ok_or_else(|| error("incomplete data in unci image"))
        };
        match &header.units {
            Some(units) if unit_type == UNIT_TILE => Ok(Source::PerTile {
                data,
                units,
                compression,
                max_bytes,
            }),
            Some(units) => {
                let mut whole = Vec::new();
                for u in units {
                    let left = max_bytes.saturating_sub(whole.len());
                    whole.extend_from_slice(&decompress(compression, unit(u)?, left)?);
                }
                Ok(Source::Whole(whole))
            }
            None => Ok(Source::Whole(decompress(compression, data, max_bytes)?)),
        }
    }

    /// `size` bytes from `start` of the (decompressed) data, for tile
    /// `tile_index`. With one compressed unit per tile, the tile's unit.
    /// Size of the data, unless it is decompressed tile by tile
    fn whole_len(&self) -> Option<usize> {
        match self {
            Source::Raw(data) => Some(data.len()),
            Source::Whole(data) => Some(data.len()),
            Source::PerTile { .. } => None,
        }
    }

    fn get(&self, tile_index: u64, start: u64, size: u64) -> Result<Cow<'_, [u8]>, HeicErrors> {
        match self {
            Source::Raw(data) => slice(data, start, size)
                .map(Cow::Borrowed)
                .ok_or_else(|| error("Uncompressed image data is too short")),
            Source::Whole(data) => slice(data, start, size)
                .map(Cow::Borrowed)
                .ok_or_else(|| error("Uncompressed image data is too short")),
            Source::PerTile {
                data,
                units,
                compression,
                max_bytes,
            } => {
                let &(offset, unit_size) = usize::try_from(tile_index)
                    .ok()
                    .and_then(|i| units.get(i))
                    .ok_or_else(|| error("no icef entry for tile index"))?;
                let unit = slice(data, offset, unit_size)
                    .ok_or_else(|| error("incomplete data in unci image"))?;
                let tile = decompress(*compression, unit, *max_bytes)?;
                if (tile.len() as u64) < size {
                    return Err(error("Uncompressed tile data is too short"));
                }
                Ok(Cow::Owned(tile))
            }
        }
    }
}

/// `size` bytes of `data` from `start`, if it has them
fn slice(data: &[u8], start: u64, size: u64) -> Option<&[u8]> {
    let start = usize::try_from(start).ok()?;
    data.get(start..start.checked_add(usize::try_from(size).ok()?)?)
}

/// Decompress `data` (`cmpC` compression type `kind`), to at most `limit`
/// bytes.
fn decompress(kind: [u8; 4], data: &[u8], limit: usize) -> Result<Vec<u8>, HeicErrors> {
    use zune_inflate::{DeflateDecoder, DeflateOptions};
    let options = DeflateOptions::default()
        .set_limit(limit)
        .set_size_hint(limit.min(1 << 20));
    let inflate_error = |e: zune_inflate::errors::InflateDecodeErrors| {
        error(format!("unci decompression failed: {:?}", e.error))
    };
    match &kind {
        b"zlib" => DeflateDecoder::new_with_options(data, options)
            .decode_zlib()
            .map_err(inflate_error),
        b"defl" => DeflateDecoder::new_with_options(data, options)
            .decode_deflate()
            .map_err(inflate_error),
        #[cfg(feature = "brotli")]
        b"brot" => {
            use std::io::Read;
            let mut out = Vec::new();
            brotli_decompressor::Decompressor::new(data, 4096)
                .take(limit as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|e| error(format!("unci Brotli decompression failed: {e}")))?;
            if out.len() > limit {
                return Err(error("unci decompressed data is too large"));
            }
            Ok(out)
        }
        #[cfg(not(feature = "brotli"))]
        b"brot" => Err(error(
            "unci item with Brotli compression (enable the `brotli` feature)",
        )),
        other => Err(error(format!(
            "unci item with unsupported compression {:?}",
            core::str::from_utf8(other).unwrap_or("????")
        ))),
    }
}

/// libheif's `UncompressedBitReader`: big-endian bits, with the start of
/// the current pixel, row and tile remembered for alignment.
struct BitReader<'a> {
    data: &'a [u8],
    /// position in bits
    pos: usize,
    pixel_start: usize,
    row_start: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            pixel_start: 0,
            row_start: 0,
        }
    }
    /// The next `n` (at most 32) bits; zero past the end of the data, as in
    /// libheif
    #[inline]
    fn get_bits(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let byte = self.pos >> 3;
        let shift = (self.pos & 7) as u32;
        self.pos += n as usize;
        if shift == 0 && n == 8 {
            return u32::from(self.data.get(byte).copied().unwrap_or(0));
        }
        let mut buf = [0u8; 8];
        let available = self.data.get(byte..).unwrap_or(&[]);
        let k = available.len().min(5);
        buf[..k].copy_from_slice(&available[..k]);
        ((u64::from_be_bytes(buf) << shift) >> (64 - n)) as u32
    }
    fn skip_bits(&mut self, n: u32) {
        self.pos += n as usize;
    }
    fn skip_bytes(&mut self, n: u64) {
        self.pos = self.pos.saturating_add((n as usize).saturating_mul(8));
    }
    fn skip_to_byte_boundary(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }
    /// Index of the byte the position is in
    fn byte_index(&self) -> usize {
        self.pos >> 3
    }
    fn mark_pixel_start(&mut self) {
        self.pixel_start = self.byte_index();
    }
    fn mark_row_start(&mut self) {
        self.row_start = self.byte_index();
    }
    /// Pad the pixel that started at `mark_pixel_start` to `pixel_size`
    /// bytes.
    fn handle_pixel_alignment(&mut self, pixel_size: u32) -> Result<(), HeicErrors> {
        if pixel_size != 0 {
            self.skip_to_byte_boundary();
            let bytes_in_pixel = (self.byte_index() - self.pixel_start) as u64;
            let pixel_size = u64::from(pixel_size);
            if pixel_size < bytes_in_pixel {
                return Err(error("Uncompressed image: invalid 'pixel_size'"));
            }
            self.skip_bytes(pixel_size - bytes_in_pixel);
        }
        Ok(())
    }
    /// Go to the next byte, then pad the row that started at
    /// `mark_row_start` to a multiple of `alignment` bytes.
    fn handle_row_alignment(&mut self, alignment: u32) {
        self.skip_to_byte_boundary();
        if alignment != 0 {
            let bytes_in_row = (self.byte_index() - self.row_start) as u64;
            self.skip_bytes(align_up(bytes_in_row, alignment) - bytes_in_row);
        }
    }
}

/// Decodes the tiles of one image into its planes (libheif's
/// `*InterleaveDecoder`s).
struct TileDecoder<'a> {
    header: &'a UnciHeader,
    entries: &'a [Entry],
    planes: &'a mut Planes,
    /// size of a tile (luma)
    tile_w: u32,
    tile_h: u32,
    num_tiles: u64,
}

impl TileDecoder<'_> {
    /// Store sample `value` of `entry` at (`x`, `y`) of its plane
    #[inline]
    fn store(&mut self, entry: &Entry, x: u32, y: u32, value: u32) {
        if let Some(channel) = entry.channel
            && let Some(plane) = &mut self.planes.planes[channel as usize]
            && let Some(sample) = plane.data.get_mut(y as usize * plane.width + x as usize)
        {
            *sample = value as u16;
        }
    }

    /// One sample of `entry`, after its alignment padding
    #[inline]
    fn read_sample(bits: &mut BitReader, entry: &Entry) -> u32 {
        if entry.align != 0 {
            bits.skip_to_byte_boundary();
            bits.skip_bits(entry.align * 8 - entry.bits);
        }
        bits.get_bits(entry.bits)
    }

    /// One row of `entry`'s samples into its plane at (`x0`, `y`), then to
    /// the next byte (libheif's `processComponentRow`)
    fn component_row(&mut self, bits: &mut BitReader, entry: &Entry, x0: u32, y: u32) {
        for x in 0..entry.tile_w {
            let value = Self::read_sample(bits, entry);
            self.store(entry, x0 + x, y, value);
        }
        bits.skip_to_byte_boundary();
    }

    fn decode_tile(
        &mut self, source: &Source, tile_index: u64, out_x0: u32, out_y0: u32,
    ) -> Result<(), HeicErrors> {
        let h = self.header;
        let entries = self.entries;
        match h.interleave {
            INTERLEAVE_COMPONENT => {
                let mut tile_size = 0u64;
                for e in entries {
                    let row = align_up(
                        (u64::from(e.padded_bits()) * u64::from(e.tile_w)).div_ceil(8),
                        h.row_align,
                    );
                    tile_size += row * u64::from(e.tile_h);
                }
                let tile_size = align_up(tile_size, h.tile_align);
                let data = source.get(tile_index, tile_size * tile_index, tile_size)?;
                let mut bits = BitReader::new(&data);
                for e in entries {
                    let (x0, y0) = (out_x0 / e.sub.0, out_y0 / e.sub.1);
                    for y in 0..e.tile_h {
                        bits.mark_row_start();
                        if e.channel.is_some() {
                            self.component_row(&mut bits, e, x0, y0 + y);
                        } else {
                            bits.skip_bytes(e.plane_row_bytes());
                        }
                        bits.handle_row_alignment(h.row_align);
                    }
                }
            }
            INTERLEAVE_PIXEL => {
                let mut bits_per_row = 0u64;
                for _ in 0..self.tile_w {
                    let mut bits_per_pixel = 0u64;
                    for e in entries {
                        if e.align > 0 {
                            bits_per_row = bits_per_row.next_multiple_of(8);
                        }
                        bits_per_pixel += u64::from(e.padded_bits());
                    }
                    if h.pixel_size != 0 {
                        let bytes = bits_per_pixel.div_ceil(8);
                        if bytes > u64::from(h.pixel_size) {
                            return Err(error("Uncompressed image: invalid 'pixel_size'"));
                        }
                        bits_per_pixel = align_up(bytes, h.pixel_size) * 8;
                    }
                    bits_per_row += bits_per_pixel;
                }
                let row = align_up(bits_per_row.div_ceil(8), h.row_align);
                let tile_size = align_up(row * u64::from(self.tile_h), h.tile_align);
                let data = source.get(tile_index, tile_size * tile_index, tile_size)?;
                let mut bits = BitReader::new(&data);
                for y in 0..self.tile_h {
                    bits.mark_row_start();
                    for x in 0..self.tile_w {
                        bits.mark_pixel_start();
                        for e in entries {
                            if e.channel.is_some() {
                                let value = Self::read_sample(&mut bits, e);
                                self.store(e, out_x0 + x, out_y0 + y, value);
                            } else {
                                bits.skip_bytes(u64::from(e.bytes));
                            }
                        }
                        bits.handle_pixel_alignment(h.pixel_size)?;
                    }
                    bits.handle_row_alignment(h.row_align);
                }
            }
            INTERLEAVE_ROW => {
                let mut bits_per_row = 0u64;
                for e in entries {
                    if e.align > 0 {
                        bits_per_row = bits_per_row.next_multiple_of(8);
                    }
                    let bits = u64::from(e.padded_bits()) * u64::from(self.tile_w);
                    bits_per_row += if h.row_align != 0 {
                        align_up(bits.div_ceil(8), h.row_align) * 8
                    } else {
                        bits
                    };
                    bits_per_row = bits_per_row.next_multiple_of(8);
                }
                let row = align_up(bits_per_row.div_ceil(8), h.row_align);
                let tile_size = align_up(row * u64::from(self.tile_h), h.tile_align);
                let data = source.get(tile_index, tile_size * tile_index, tile_size)?;
                let mut bits = BitReader::new(&data);
                for y in 0..self.tile_h {
                    for e in entries {
                        bits.mark_row_start();
                        if e.channel.is_some() {
                            self.component_row(&mut bits, e, out_x0, out_y0 + y);
                        } else {
                            bits.skip_bytes(e.plane_row_bytes());
                        }
                        bits.handle_row_alignment(h.row_align);
                    }
                }
            }
            INTERLEAVE_MIXED => {
                // luma rows, then Cb and Cr interleaved, each sample in
                // whole bytes
                let mut tile_size = 0u64;
                for e in entries {
                    let bits = if matches!(e.channel, Some(Channel::Cb | Channel::Cr)) {
                        e.bits
                    } else {
                        e.padded_bits()
                    };
                    let row = (u64::from(bits) * u64::from(e.tile_w)).div_ceil(8);
                    tile_size += row * u64::from(e.tile_h);
                }
                let tile_size = align_up(tile_size, h.tile_align);
                let data = source.get(tile_index, tile_size * tile_index, tile_size)?;
                let mut bits = BitReader::new(&data);
                let mut chroma_done = false;
                for e in entries {
                    let (x0, y0) = (out_x0 / e.sub.0, out_y0 / e.sub.1);
                    match e.channel {
                        Some(channel @ (Channel::Cb | Channel::Cr)) => {
                            if chroma_done {
                                continue;
                            }
                            chroma_done = true;
                            let other_channel =
                                if channel == Channel::Cb { Channel::Cr } else { Channel::Cb };
                            let other = Entry {
                                channel: Some(other_channel),
                                ..*e
                            };
                            for y in 0..e.tile_h {
                                for x in 0..e.tile_w {
                                    let value = bits.get_bits(e.bytes * 8);
                                    self.store(e, x0 + x, y0 + y, value);
                                    let value = bits.get_bits(e.bytes * 8);
                                    self.store(&other, x0 + x, y0 + y, value);
                                }
                            }
                        }
                        Some(_) => {
                            for y in 0..e.tile_h {
                                self.component_row(&mut bits, e, x0, y0 + y);
                            }
                        }
                        None => bits.skip_bytes(e.plane_row_bytes() * u64::from(e.tile_h)),
                    }
                }
            }
            INTERLEAVE_TILE_COMPONENT => {
                // each component of all tiles, then the next component
                let mut component_start = 0u64;
                for e in entries {
                    let row = align_up(
                        (u64::from(e.padded_bits()) * u64::from(self.tile_w)).div_ceil(8),
                        h.row_align,
                    );
                    let size = align_up(row * u64::from(self.tile_h), h.tile_align);
                    if e.channel.is_some() {
                        let data =
                            source.get(tile_index, component_start + size * tile_index, size)?;
                        let mut bits = BitReader::new(&data);
                        for y in 0..e.tile_h {
                            bits.mark_row_start();
                            self.component_row(&mut bits, e, out_x0, out_y0 + y);
                            bits.handle_row_alignment(h.row_align);
                        }
                    }
                    component_start += size * self.num_tiles;
                }
            }
            other => {
                return Err(error(format!(
                    "Uncompressed interleave type {other} is not supported"
                )));
            }
        }
        Ok(())
    }
}
