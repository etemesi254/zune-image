//! `unci` (uncompressed, ISO/IEC 23001-17) images: files are built here with
//! known samples, in the layouts the standard defines, and the decoded
//! pixels are checked against those samples.
//!
//! The same files decode identically with libheif 1.21 (set
//! `ZUNE_HEIC_UNCI_DUMP` to a directory to write them out).
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_heic::HeifDecoder;

// component types (cmpd)
const MONO: u16 = 0;
const Y: u16 = 1;
const CB: u16 = 2;
const CR: u16 = 3;
const R: u16 = 4;
const G: u16 = 5;
const B: u16 = 6;
const A: u16 = 7;
const PAD: u16 = 12;

fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut b = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend_from_slice(kind);
    b.extend_from_slice(payload);
    b
}

fn full_box(kind: &[u8; 4], version: u8, payload: &[u8]) -> Vec<u8> {
    let mut p = vec![version, 0, 0, 0];
    p.extend_from_slice(payload);
    bx(kind, &p)
}

fn cmpd(types: &[u16]) -> Vec<u8> {
    let mut p = (types.len() as u32).to_be_bytes().to_vec();
    for t in types {
        p.extend_from_slice(&t.to_be_bytes());
    }
    bx(b"cmpd", &p)
}

/// `uncC` version 0
#[derive(Clone)]
struct UncC {
    /// (bit depth, component_align_size) of each `cmpd` component, in order
    components: Vec<(u8, u8)>,
    sampling: u8,
    interleave: u8,
    pixel_size: u32,
    row_align: u32,
    tile_align: u32,
    tiles: (u32, u32),
}

impl UncC {
    fn new(components: &[(u8, u8)], interleave: u8) -> Self {
        Self {
            components: components.to_vec(),
            sampling: 0,
            interleave,
            pixel_size: 0,
            row_align: 0,
            tile_align: 0,
            tiles: (1, 1),
        }
    }
    fn to_box(&self) -> Vec<u8> {
        let mut p = vec![0u8; 4]; // profile
        p.extend_from_slice(&(self.components.len() as u32).to_be_bytes());
        for (i, &(bits, align)) in self.components.iter().enumerate() {
            p.extend_from_slice(&(i as u16).to_be_bytes());
            p.extend_from_slice(&[bits - 1, 0, align]);
        }
        p.extend_from_slice(&[self.sampling, self.interleave, 0, 0]);
        for v in [
            self.pixel_size,
            self.row_align,
            self.tile_align,
            self.tiles.0 - 1,
            self.tiles.1 - 1,
        ] {
            p.extend_from_slice(&v.to_be_bytes());
        }
        full_box(b"uncC", 0, &p)
    }
}

/// A HEIF file whose primary item is a `unci` image with `properties` (plus
/// `ispe`) and `data`.
fn heif(width: u32, height: u32, properties: &[Vec<u8>], data: &[u8]) -> Vec<u8> {
    let ftyp = bx(b"ftyp", b"mif1\0\0\0\0mif1");
    let meta = |data_offset: u32| {
        let hdlr = full_box(b"hdlr", 0, b"\0\0\0\0pict\0\0\0\0\0\0\0\0\0\0\0\0\0");
        let pitm = full_box(b"pitm", 0, &1u16.to_be_bytes());
        let infe = full_box(b"infe", 2, b"\0\x01\0\0unci\0");
        let mut iinf = 1u16.to_be_bytes().to_vec();
        iinf.extend_from_slice(&infe);
        let iinf = full_box(b"iinf", 0, &iinf);
        let mut iloc = vec![0x44, 0x00, 0, 1, 0, 1, 0, 0, 0, 1];
        iloc.extend_from_slice(&data_offset.to_be_bytes());
        iloc.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let iloc = full_box(b"iloc", 0, &iloc);
        let mut ispe = width.to_be_bytes().to_vec();
        ispe.extend_from_slice(&height.to_be_bytes());
        let mut ipco = full_box(b"ispe", 0, &ispe);
        for p in properties {
            ipco.extend_from_slice(p);
        }
        let n = properties.len() + 1;
        let mut ipma = vec![0, 0, 0, 1, 0, 1, n as u8];
        ipma.extend((1..=n as u8).map(|i| 0x80 | i));
        let mut iprp = bx(b"ipco", &ipco);
        iprp.extend_from_slice(&full_box(b"ipma", 0, &ipma));
        let iprp = bx(b"iprp", &iprp);
        full_box(b"meta", 0, &[hdlr, pitm, iinf, iloc, iprp].concat())
    };
    let meta_len = meta(0).len();
    let data_offset = (ftyp.len() + meta_len + 8) as u32;
    [ftyp, meta(data_offset), bx(b"mdat", data)].concat()
}

/// Packs samples most significant bit first.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    bits: usize,
}

impl Bits {
    fn put(&mut self, value: u32, n: u32) {
        for i in (0..n).rev() {
            if self.bits.is_multiple_of(8) {
                self.bytes.push(0);
            }
            if (value >> i) & 1 == 1 {
                *self.bytes.last_mut().unwrap() |= 0x80 >> (self.bits % 8);
            }
            self.bits += 1;
        }
    }
    fn byte_align(&mut self) {
        self.bits = self.bits.next_multiple_of(8);
    }
    /// pad with zero bytes to a multiple of `n` bytes from `start`
    fn pad_from(&mut self, start: usize, n: usize) {
        self.byte_align();
        while !(self.bytes.len() - start).is_multiple_of(n) {
            self.bytes.push(0);
            self.bits += 8;
        }
    }
}

/// The test sample of component `c` at (`x`, `y`), `bits` wide
fn sample(c: usize, x: usize, y: usize, bits: u8) -> u32 {
    ((x * 37 + y * 101 + c * 59 + x * y * 7) as u32) & ((1u32 << bits) - 1)
}

/// `v` of `bits` as an 8-bit sample, as libheif converts it
fn to_8bit(v: u32, bits: u8) -> u8 {
    match bits {
        8 => v as u8,
        b if b > 8 => (v >> (b - 8)) as u8,
        b => {
            // repeat the bits
            let mut out = 0u32;
            let mut filled = 0;
            while filled < 8 {
                out = (out << b) | v;
                filled += b;
            }
            (out >> (filled - 8)) as u8
        }
    }
}

fn decode(file: &[u8]) -> (usize, usize, ColorSpace, Vec<u8>) {
    let mut d = HeifDecoder::new(ZCursor::new(file));
    let pixels = d.decode().expect("decode failed");
    (
        d.width().unwrap(),
        d.height().unwrap(),
        d.colorspace().unwrap(),
        pixels,
    )
}

fn dump(name: &str, file: &[u8]) {
    if let Some(dir) = std::env::var_os("ZUNE_HEIC_UNCI_DUMP") {
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.heif")),
            file,
        )
        .unwrap();
    }
}

/// Decode `file` and check that each output channel of each pixel holds
/// `expected(channel, x, y)`.
fn check(
    name: &str, file: &[u8], size: (usize, usize), colorspace: ColorSpace,
    expected: impl Fn(usize, usize, usize) -> u8,
) {
    dump(name, file);
    let (w, h, cs, pixels) = decode(file);
    assert_eq!((w, h, cs), (size.0, size.1, colorspace), "{name}");
    let n = cs.num_components();
    for y in 0..h {
        for x in 0..w {
            for c in 0..n {
                assert_eq!(
                    pixels[(y * w + x) * n + c],
                    expected(c, x, y),
                    "{name}: channel {c} of pixel ({x}, {y})"
                );
            }
        }
    }
}

/// Component interleave: each component of a tile after the other, rows
/// padded to `row_align`, tiles to `tile_align`
fn component_data(unc: &UncC, w: usize, h: usize) -> Vec<u8> {
    let (tw, th) = (w / unc.tiles.0 as usize, h / unc.tiles.1 as usize);
    let mut bits = Bits::default();
    for ty in 0..unc.tiles.1 as usize {
        for tx in 0..unc.tiles.0 as usize {
            let tile_start = bits.bytes.len();
            for (c, &(depth, _)) in unc.components.iter().enumerate() {
                for y in 0..th {
                    let row_start = bits.bytes.len();
                    for x in 0..tw {
                        bits.put(sample(c, tx * tw + x, ty * th + y, depth), u32::from(depth));
                    }
                    bits.pad_from(row_start, unc.row_align.max(1) as usize);
                }
            }
            bits.pad_from(tile_start, unc.tile_align.max(1) as usize);
        }
    }
    bits.bytes
}

#[test]
fn component_interleave_tiled() {
    let (w, h) = (8, 6);
    let mut unc = UncC::new(&[(8, 0), (8, 0), (8, 0)], 0);
    unc.tiles = (2, 3);
    unc.row_align = 3;
    unc.tile_align = 5;
    let data = component_data(&unc, w, h);
    let file = heif(w as u32, h as u32, &[cmpd(&[R, G, B]), unc.to_box()], &data);
    check(
        "component_rgb_tiled",
        &file,
        (w, h),
        ColorSpace::RGB,
        |c, x, y| sample(c, x, y, 8) as u8,
    );
}

#[test]
fn component_interleave_bgr_16_bit() {
    let (w, h) = (5, 3);
    let unc = UncC::new(&[(16, 0), (16, 0), (16, 0)], 0);
    let data = component_data(&unc, w, h);
    let file = heif(w as u32, h as u32, &[cmpd(&[B, R, G]), unc.to_box()], &data);
    // stored B, R, G
    let order = [1, 2, 0];
    check(
        "component_bgr16",
        &file,
        (w, h),
        ColorSpace::RGB,
        |c, x, y| to_8bit(sample(order[c], x, y, 16), 16),
    );
}

#[test]
fn component_interleave_3_bit() {
    // fewer than 8 bits: rows are bit-packed, then byte aligned
    let (w, h) = (7, 3);
    let unc = UncC::new(&[(3, 0), (3, 0), (3, 0)], 0);
    let data = component_data(&unc, w, h);
    let file = heif(w as u32, h as u32, &[cmpd(&[R, G, B]), unc.to_box()], &data);
    check(
        "component_rgb3bit",
        &file,
        (w, h),
        ColorSpace::RGB,
        |c, x, y| to_8bit(sample(c, x, y, 3), 3),
    );
}

#[test]
fn pixel_interleave_with_padding() {
    // RGBA, each pixel padded to 6 bytes, rows to a multiple of 4
    let (w, h) = (5, 4);
    let mut unc = UncC::new(&[(8, 0), (8, 0), (8, 0), (8, 0)], 1);
    unc.pixel_size = 6;
    unc.row_align = 4;
    let mut bits = Bits::default();
    for y in 0..h {
        let row_start = bits.bytes.len();
        for x in 0..w {
            let pixel_start = bits.bytes.len();
            for c in 0..4 {
                bits.put(sample(c, x, y, 8), 8);
            }
            bits.pad_from(pixel_start, 6);
        }
        bits.pad_from(row_start, 4);
    }
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[R, G, B, A]), unc.to_box()],
        &bits.bytes,
    );
    check(
        "pixel_rgba_padded",
        &file,
        (w, h),
        ColorSpace::RGBA,
        |c, x, y| sample(c, x, y, 8) as u8,
    );
}

#[test]
fn pixel_interleave_r5g6b5() {
    let (w, h) = (6, 2);
    let depths = [5, 6, 5];
    let unc = UncC::new(&[(5, 0), (6, 0), (5, 0)], 1);
    let mut bits = Bits::default();
    for y in 0..h {
        for x in 0..w {
            for (c, &d) in depths.iter().enumerate() {
                bits.put(sample(c, x, y, d), u32::from(d));
            }
        }
        bits.byte_align();
    }
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[R, G, B]), unc.to_box()],
        &bits.bytes,
    );
    check("pixel_r5g6b5", &file, (w, h), ColorSpace::RGB, |c, x, y| {
        to_8bit(sample(c, x, y, depths[c]), depths[c])
    });
}

#[test]
fn pixel_interleave_padding_component() {
    // R, G, a padding byte, B
    let (w, h) = (4, 3);
    let unc = UncC::new(&[(8, 0), (8, 0), (8, 0), (8, 0)], 1);
    let mut bits = Bits::default();
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                bits.put(if c == 2 { 0xAA } else { sample(c, x, y, 8) }, 8);
            }
        }
    }
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[R, G, PAD, B]), unc.to_box()],
        &bits.bytes,
    );
    let source = [0, 1, 3];
    check("pixel_rgxb", &file, (w, h), ColorSpace::RGB, |c, x, y| {
        sample(source[c], x, y, 8) as u8
    });
}

#[test]
fn row_interleave_grey_16_bit_with_alpha() {
    // a row of 16-bit grey, then a row of 8-bit alpha
    let (w, h) = (5, 4);
    let unc = UncC::new(&[(16, 0), (8, 0)], 3);
    let mut bits = Bits::default();
    for y in 0..h {
        for x in 0..w {
            bits.put(sample(0, x, y, 16), 16);
        }
        for x in 0..w {
            bits.put(sample(1, x, y, 8), 8);
        }
    }
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[MONO, A]), unc.to_box()],
        &bits.bytes,
    );
    check(
        "row_grey16_alpha",
        &file,
        (w, h),
        ColorSpace::LumaA,
        |c, x, y| {
            if c == 0 { to_8bit(sample(0, x, y, 16), 16) } else { sample(1, x, y, 8) as u8 }
        },
    );
}

#[test]
fn row_interleave_component_alignment() {
    // 7-bit components, each in a byte of its own (component_align_size 1)
    let (w, h) = (4, 2);
    let mut unc = UncC::new(&[(7, 1), (7, 1), (7, 1)], 3);
    unc.tiles = (2, 1);
    let tw = w / 2;
    let mut bits = Bits::default();
    for tx in 0..2 {
        for y in 0..h {
            for c in 0..3 {
                for x in 0..tw {
                    bits.put(sample(c, tx * tw + x, y, 7), 8);
                }
            }
        }
    }
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[R, G, B]), unc.to_box()],
        &bits.bytes,
    );
    check(
        "row_rgb7_aligned_tiled",
        &file,
        (w, h),
        ColorSpace::RGB,
        |c, x, y| to_8bit(sample(c, x, y, 7), 7),
    );
}

#[test]
fn tile_component_interleave() {
    // component 0 of every tile, then component 1 of every tile, ...
    let (w, h) = (6, 4);
    let mut unc = UncC::new(&[(8, 0), (8, 0), (8, 0)], 4);
    unc.tiles = (3, 2);
    let (tw, th) = (2, 2);
    let mut bits = Bits::default();
    for c in 0..3 {
        for ty in 0..2 {
            for tx in 0..3 {
                for y in 0..th {
                    for x in 0..tw {
                        bits.put(sample(c, tx * tw + x, ty * th + y, 8), 8);
                    }
                }
            }
        }
    }
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[R, G, B]), unc.to_box()],
        &bits.bytes,
    );
    check(
        "tile_component_rgb",
        &file,
        (w, h),
        ColorSpace::RGB,
        |c, x, y| sample(c, x, y, 8) as u8,
    );
}

#[test]
fn ycbcr_420_component_interleave() {
    // grey chroma: the output is the luma in every channel (full range)
    let (w, h) = (6, 4);
    let mut unc = UncC::new(&[(8, 0), (8, 0), (8, 0)], 0);
    unc.sampling = 2;
    let mut bits = Bits::default();
    for y in 0..h {
        for x in 0..w {
            bits.put(sample(0, x, y, 8), 8);
        }
    }
    for _ in 0..2 * (w / 2) * (h / 2) {
        bits.put(128, 8);
    }
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[Y, CB, CR]), unc.to_box()],
        &bits.bytes,
    );
    check("ycbcr420", &file, (w, h), ColorSpace::RGB, |_, x, y| {
        sample(0, x, y, 8) as u8
    });
}

#[test]
fn ycbcr_422_mixed_interleave() {
    // luma rows, then Cb and Cr interleaved
    let (w, h) = (6, 3);
    let mut unc = UncC::new(&[(8, 0), (8, 0), (8, 0)], 2);
    unc.sampling = 1;
    let mut bits = Bits::default();
    for y in 0..h {
        for x in 0..w {
            bits.put(sample(0, x, y, 8), 8);
        }
    }
    for _ in 0..(w / 2) * h {
        bits.put(128, 8);
        bits.put(128, 8);
    }
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[Y, CB, CR]), unc.to_box()],
        &bits.bytes,
    );
    check(
        "ycbcr422_mixed",
        &file,
        (w, h),
        ColorSpace::RGB,
        |_, x, y| sample(0, x, y, 8) as u8,
    );
}

#[test]
fn profile_rgba() {
    // uncC version 1: a profile instead of the component list, no cmpd
    let (w, h) = (3, 2);
    let mut data = Vec::new();
    for y in 0..h {
        for x in 0..w {
            data.extend((0..4).map(|c| sample(c, x, y, 8) as u8));
        }
    }
    let unc = full_box(b"uncC", 1, b"rgba");
    let file = heif(w as u32, h as u32, &[unc], &data);
    check(
        "profile_rgba",
        &file,
        (w, h),
        ColorSpace::RGBA,
        |c, x, y| sample(c, x, y, 8) as u8,
    );
}

fn zlib(data: &[u8]) -> Vec<u8> {
    zune_inflate::DeflateEncoder::new(data).encode_zlib()
}

fn cmp_c(kind: &[u8; 4], unit_type: u8) -> Vec<u8> {
    let mut p = kind.to_vec();
    p.push(unit_type);
    full_box(b"cmpC", 0, &p)
}

/// `icef` with 32-bit offsets and sizes
fn icef(units: &[(u32, u32)]) -> Vec<u8> {
    let mut p = vec![(3 << 5) | (3 << 2)];
    p.extend_from_slice(&(units.len() as u32).to_be_bytes());
    for (offset, size) in units {
        p.extend_from_slice(&offset.to_be_bytes());
        p.extend_from_slice(&size.to_be_bytes());
    }
    full_box(b"icef", 0, &p)
}

#[test]
fn zlib_compressed_image() {
    let (w, h) = (9, 5);
    let unc = UncC::new(&[(8, 0), (8, 0), (8, 0)], 0);
    let data = zlib(&component_data(&unc, w, h));
    let file = heif(
        w as u32,
        h as u32,
        &[cmpd(&[R, G, B]), unc.to_box(), cmp_c(b"zlib", 0)],
        &data,
    );
    check("zlib", &file, (w, h), ColorSpace::RGB, |c, x, y| {
        sample(c, x, y, 8) as u8
    });
}

#[test]
fn deflate_compressed_tiles() {
    // one compressed unit per tile, listed in `icef`
    let (w, h) = (8, 4);
    let mut unc = UncC::new(&[(8, 0), (8, 0), (8, 0)], 0);
    unc.tiles = (2, 2);
    let raw = component_data(&unc, w, h);
    let tile_size = raw.len() / 4;
    let (mut data, mut units) = (Vec::new(), Vec::new());
    for tile in raw.chunks(tile_size) {
        let z = zlib(tile);
        // raw deflate: without the zlib header and checksum
        let deflate = &z[2..z.len() - 4];
        units.push((data.len() as u32, deflate.len() as u32));
        data.extend_from_slice(deflate);
    }
    let properties = [
        cmpd(&[R, G, B]),
        unc.to_box(),
        cmp_c(b"defl", 2),
        icef(&units),
    ];
    let file = heif(w as u32, h as u32, &properties, &data);
    check(
        "deflate_tiles",
        &file,
        (w, h),
        ColorSpace::RGB,
        |c, x, y| sample(c, x, y, 8) as u8,
    );
}

#[test]
fn unsupported_layouts_fail() {
    let (w, h) = (4, 4);
    // blocks
    let unc = UncC::new(&[(8, 0)], 0);
    let mut unc_box = unc.to_box();
    // header, version and flags, profile, count, one component, sampling,
    // interleave
    unc_box[8 + 4 + 4 + 4 + 5 + 2] = 4;
    let file = heif(w, h, &[cmpd(&[MONO]), unc_box], &[0; 16]);
    assert!(
        HeifDecoder::new(ZCursor::new(file.as_slice()))
            .decode()
            .is_err()
    );

    // floating point components
    let mut unc_box = UncC::new(&[(32, 0)], 0).to_box();
    // header, version and flags, profile, count, index, bit depth
    unc_box[8 + 4 + 4 + 4 + 2 + 1] = 1;
    let file = heif(w, h, &[cmpd(&[MONO]), unc_box], &[0; 64]);
    assert!(
        HeifDecoder::new(ZCursor::new(file.as_slice()))
            .decode()
            .is_err()
    );

    // too little data
    let unc = UncC::new(&[(8, 0), (8, 0), (8, 0)], 0);
    let file = heif(w, h, &[cmpd(&[R, G, B]), unc.to_box()], &[0; 47]);
    assert!(
        HeifDecoder::new(ZCursor::new(file.as_slice()))
            .decode()
            .is_err()
    );
}
