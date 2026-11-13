//! The color encoding an image header names, from zune-core's color
//! description: [`ColorCharacteristics`] for the transfer function and
//! [`ColorPrimaries`] for the primaries and white point, as the JPEG XL
//! codestream's `ColorEncoding` bundle (ISO/IEC 18181-1) holds them,
//! with the relative rendering intent.
//!
//! Primaries and white points the codestream has a name for (sRGB's,
//! BT.2100's and P3's primaries; D65, DCI and E) are written as that
//! name, and any others as their CIE xy chromaticities.

use zune_core::colorspace::{ColorCharacteristics, ColorPrimaries, SingleColorPrimary};

use crate::errors::JxlEncodeErrors;

/// How far a chromaticity may be from a named one and be written as
/// that name: past what PNG's `cHRM`, at five decimals, rounds away.
const NAMED_WITHIN: f64 = 5e-4;

/// The codestream's scale for a custom chromaticity: millionths.
const XY_SCALE: f64 = 1_000_000.0;

/// The most a custom chromaticity's coordinate may be either way, in
/// millionths: the largest its packed field holds, 2²² − 1, unpacked.
const XY_MOST: i32 = 2_097_151;

/// The codestream's scale for a gamma: ten-millionths of the exponent
/// that encodes linear light.
const GAMMA_SCALE: f64 = 10_000_000.0;

/// sRGB's, BT.709's, primaries: red, green and blue.
const SRGB: [(f64, f64); 3] = [(0.64, 0.33), (0.30, 0.60), (0.15, 0.06)];

/// BT.2100's, BT.2020's, primaries.
const BT2100: [(f64, f64); 3] = [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)];

/// P3's primaries.
const P3: [(f64, f64); 3] = [(0.680, 0.320), (0.265, 0.690), (0.150, 0.060)];

/// The D65 white point.
const D65: (f64, f64) = (0.3127, 0.3290);

/// DCI-P3's white point.
const DCI: (f64, f64) = (0.314, 0.351);

/// The equal-energy white point, E.
const E: (f64, f64) = (1.0 / 3.0, 1.0 / 3.0);

/// A white point or set of primaries, by the codestream's name for it
/// or by its chromaticities.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Named<T> {
    /// The codestream's enum value.
    Value(u32),
    /// Custom: each chromaticity in millionths.
    Custom(T),
}

/// A chromaticity, x and y in millionths.
pub(crate) type Xy = (i32, i32);

/// A transfer function as the codestream holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Transfer {
    /// The codestream's enum value.
    Value(u32),
    /// A gamma: the exponent encoding linear light, in ten-millionths.
    Gamma(u32),
}

/// What the image header says of the samples' color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ColorEncoding {
    /// The white point.
    pub(crate) white: Named<Xy>,
    /// The primaries, red, green and blue, which grayscale leaves out.
    pub(crate) primaries: Named<[Xy; 3]>,
    /// The transfer function.
    pub(crate) transfer: Transfer,
}

impl ColorEncoding {
    /// sRGB's, which the header writes as `all_default`.
    pub(crate) const SRGB: ColorEncoding = ColorEncoding {
        white: Named::Value(1),
        primaries: Named::Value(1),
        transfer: Transfer::Value(13),
    };

    /// The encoding of `transfer` and `primaries`, sRGB's primaries and
    /// D65 where `primaries` is `None`.
    ///
    /// # Errors
    ///
    /// [`JxlEncodeErrors::Generic`] for a transfer function the
    /// codestream has no name or gamma for, or a chromaticity past
    /// ±2.097151 that it has no room for.
    pub(crate) fn of(
        transfer: ColorCharacteristics, primaries: Option<&ColorPrimaries>,
    ) -> Result<ColorEncoding, JxlEncodeErrors> {
        let transfer = match transfer {
            ColorCharacteristics::Rec709 => Transfer::Value(1),
            ColorCharacteristics::Linear => Transfer::Value(8),
            ColorCharacteristics::sRGB => Transfer::Value(13),
            ColorCharacteristics::PQ => Transfer::Value(16),
            ColorCharacteristics::Smpte428 => Transfer::Value(17),
            ColorCharacteristics::HLG => Transfer::Value(18),
            ColorCharacteristics::Gamma2p2 => Transfer::Gamma(gamma(2.2)),
            ColorCharacteristics::Gamma2p8 => Transfer::Gamma(gamma(2.8)),
            _ => {
                return Err(JxlEncodeErrors::Generic(
                    "JPEG XL has no name for this transfer function",
                ))
            }
        };
        let Some(primaries) = primaries else {
            return Ok(ColorEncoding {
                transfer,
                ..ColorEncoding::SRGB
            });
        };
        let white = xy(&primaries.white_point);
        let rgb = [primaries.red, primaries.green, primaries.blue].map(|p| xy(&p));
        let white = [(D65, 1), (DCI, 11), (E, 10)]
            .into_iter()
            .find(|(named, _)| near(white, *named))
            .map_or(Named::Custom(millionths(white)), |(_, value)| {
                Named::Value(value)
            });
        let primaries = [(SRGB, 1), (BT2100, 9), (P3, 11)]
            .into_iter()
            .find(|(named, _)| (0..3).all(|i| near(rgb[i], named[i])))
            .map_or(Named::Custom(rgb.map(millionths)), |(_, value)| {
                Named::Value(value)
            });
        let custom_white = match white {
            Named::Custom(xy) => Some(xy),
            Named::Value(_) => None,
        };
        let custom_rgb = match primaries {
            Named::Custom(rgb) => rgb.map(Some),
            Named::Value(_) => [None; 3],
        };
        let fits = |(x, y): Xy| x.abs() <= XY_MOST && y.abs() <= XY_MOST;
        if !(custom_white.into_iter())
            .chain(custom_rgb.into_iter().flatten())
            .all(fits)
        {
            return Err(JxlEncodeErrors::Generic(
                "a chromaticity is past what JPEG XL can hold",
            ));
        }
        Ok(ColorEncoding {
            white,
            primaries,
            transfer,
        })
    }

    /// Writes the bundle after `all_default`, each field as `put` takes
    /// a count of bits and their value: RGB's with its primaries,
    /// grayscale's without.
    pub(crate) fn write(&self, rgb: bool, put: &mut dyn FnMut(u8, u64)) {
        put(1, 0); // want_icc false
        put_enum(put, if rgb { 0 } else { 1 }); // RGB or grayscale
        match self.white {
            Named::Value(value) => put_enum(put, value),
            Named::Custom(white) => {
                put_enum(put, 2);
                put_xy(put, white);
            }
        }
        if rgb {
            match self.primaries {
                Named::Value(value) => put_enum(put, value),
                Named::Custom(rgb) => {
                    put_enum(put, 2);
                    for xy in rgb {
                        put_xy(put, xy);
                    }
                }
            }
        }
        match self.transfer {
            Transfer::Gamma(gamma) => {
                put(1, 1); // have_gamma
                put(24, u64::from(gamma));
            }
            Transfer::Value(value) => {
                put(1, 0);
                put_enum(put, value);
            }
        }
        put_enum(put, 1); // relative rendering intent
    }
}

/// `primary`'s chromaticity.
fn xy(primary: &SingleColorPrimary) -> (f64, f64) {
    (primary.x, primary.y)
}

/// Whether `a` is `b`, within [`NAMED_WITHIN`] each way.
fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= NAMED_WITHIN && (a.1 - b.1).abs() <= NAMED_WITHIN
}

/// `xy` in millionths.
fn millionths((x, y): (f64, f64)) -> Xy {
    let scaled = |v: f64| (v * XY_SCALE).round() as i32;
    (scaled(x), scaled(y))
}

/// The codestream's gamma for a curve of exponent `exponent`: its
/// inverse in ten-millionths.
fn gamma(exponent: f64) -> u32 {
    (GAMMA_SCALE / exponent).round() as u32
}

/// Writes `value` as the codestream's `Enum` field codes it.
fn put_enum(put: &mut dyn FnMut(u8, u64), value: u32) {
    for (count, bits) in enum_bits(value) {
        if count > 0 {
            put(count, bits);
        }
    }
}

/// Writes a custom chromaticity, x then y, each signed value packed as
/// the codestream's `U32(Bits(19), BitsOffset(19, 524288),
/// BitsOffset(20, 1048576), BitsOffset(21, 2097152))`.
fn put_xy(put: &mut dyn FnMut(u8, u64), (x, y): Xy) {
    for value in [x, y] {
        let packed =
            if value >= 0 { 2 * value.unsigned_abs() } else { 2 * value.unsigned_abs() - 1 };
        let (selector, count, offset) = match packed {
            0..=524_287 => (0, 19, 0),
            524_288..=1_048_575 => (1, 19, 524_288),
            1_048_576..=2_097_151 => (2, 20, 1_048_576),
            _ => (3, 21, 2_097_152),
        };
        put(2, selector);
        put(count, u64::from(packed - offset));
    }
}

/// An enum value's bits as the codestream's `Enum` field codes it, a
/// `U32(Val(0), Val(1), BitsOffset(4, 2), BitsOffset(6, 18))`: the
/// selector's 2 bits, then any extra bits, each as `(count, value)`.
pub(crate) fn enum_bits(value: u32) -> [(u8, u64); 2] {
    match value {
        0 => [(2, 0), (0, 0)],
        1 => [(2, 1), (0, 0)],
        2..=17 => [(2, 2), (4, u64::from(value - 2))],
        _ => [(2, 3), (6, u64::from(value - 18))],
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use zune_core::bit_depth::BitDepth;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::EncoderOptions;

    use super::*;
    use crate::JxlSimpleEncoder;

    /// The codestream's bits, least significant first.
    struct Bits<'a> {
        data: &'a [u8],
        at: usize,
    }

    impl Bits<'_> {
        fn read(&mut self, count: usize) -> u32 {
            let mut value = 0;
            for i in 0..count {
                let bit = (self.data[self.at / 8] >> (self.at % 8)) & 1;
                value |= u32::from(bit) << i;
                self.at += 1;
            }
            value
        }

        fn read_enum(&mut self) -> u32 {
            match self.read(2) {
                0 => 0,
                1 => 1,
                2 => 2 + self.read(4),
                _ => 18 + self.read(6),
            }
        }

        /// A custom chromaticity's coordinate, in millionths.
        fn read_coordinate(&mut self) -> i32 {
            let packed = match self.read(2) {
                0 => self.read(19),
                1 => 524_288 + self.read(19),
                2 => 1_048_576 + self.read(20),
                _ => 2_097_152 + self.read(21),
            };
            if packed % 2 == 0 {
                (packed / 2) as i32
            } else {
                -((packed / 2) as i32) - 1
            }
        }

        fn read_xy(&mut self) -> Xy {
            (self.read_coordinate(), self.read_coordinate())
        }

        fn read_named<T>(&mut self, custom: impl FnOnce(&mut Self) -> T) -> Named<T> {
            match self.read_enum() {
                2 => Named::Custom(custom(self)),
                value => Named::Value(value),
            }
        }
    }

    /// What the header of a 2 × 2 8-bit image of `colorspace`, its
    /// transfer `trc` and primaries `primaries`, says of its color:
    /// `None` where it is all_default, else whether it is RGB and its
    /// encoding, grayscale's primaries left as sRGB's.
    fn header(
        colorspace: ColorSpace, trc: ColorCharacteristics, primaries: Option<ColorPrimaries>,
    ) -> Option<(bool, ColorEncoding)> {
        let channels = colorspace.num_components();
        let data = alloc::vec![100_u8; 4 * channels];
        let options = EncoderOptions::new(2, 2, colorspace, BitDepth::Eight);
        let mut encoder = JxlSimpleEncoder::new(&data, options);
        encoder.set_color_characteristics(trc);
        if let Some(primaries) = primaries {
            encoder.set_color_primaries(primaries);
        }
        let mut out = Vec::new();
        encoder.encode(&mut out).unwrap();
        let mut bits = Bits { data: &out, at: 0 };
        assert_eq!(bits.read(16), 0x0AFF, "signature");
        assert_eq!(bits.read(1), 0, "not small");
        assert_eq!((bits.read(2), bits.read(9)), (0, 1), "height 2");
        assert_eq!(bits.read(3), 0, "no ratio");
        assert_eq!((bits.read(2), bits.read(9)), (0, 1), "width 2");
        assert_eq!(bits.read(1), 0, "metadata not all_default");
        assert_eq!(bits.read(1), 0, "no extra fields");
        assert_eq!(bits.read(1), 0, "integer samples");
        assert_eq!(bits.read(2), 0, "8 bits");
        assert_eq!(bits.read(1), 1, "16-bit buffer suffices");
        assert_eq!(
            bits.read(2),
            u32::from(colorspace.has_alpha()),
            "extra channels"
        );
        if colorspace.has_alpha() {
            assert_eq!(bits.read(1), 1, "default alpha");
        }
        assert_eq!(bits.read(1), 0, "not xyb");
        if bits.read(1) == 1 {
            return None;
        }
        assert_eq!(bits.read(1), 0, "no ICC");
        let rgb = match bits.read_enum() {
            0 => true,
            1 => false,
            other => panic!("color space {other}"),
        };
        let white = bits.read_named(Bits::read_xy);
        let primaries = if rgb {
            bits.read_named(|b| [b.read_xy(), b.read_xy(), b.read_xy()])
        } else {
            Named::Value(1)
        };
        let transfer = if bits.read(1) == 1 {
            Transfer::Gamma(bits.read(24))
        } else {
            Transfer::Value(bits.read_enum())
        };
        assert_eq!(bits.read_enum(), 1, "relative intent");
        Some((
            rgb,
            ColorEncoding {
                white,
                primaries,
                transfer,
            },
        ))
    }

    /// Primaries of red, green and blue and a white point, from their
    /// chromaticities.
    fn primaries(rgb: [(f64, f64); 3], white: (f64, f64)) -> ColorPrimaries {
        let point = |(x, y): (f64, f64)| SingleColorPrimary { x, y, z: 0.0 };
        ColorPrimaries {
            red: point(rgb[0]),
            green: point(rgb[1]),
            blue: point(rgb[2]),
            white_point: point(white),
        }
    }

    #[test]
    fn srgb_is_all_default_and_named_primaries_and_transfers_are_named() {
        use ColorCharacteristics as C;
        for colorspace in [ColorSpace::RGB, ColorSpace::RGBA] {
            assert_eq!(header(colorspace, C::sRGB, None), None);
            assert_eq!(
                header(colorspace, C::sRGB, Some(primaries(SRGB, D65))),
                None
            );
            for (named, p) in [(SRGB, 1), (BT2100, 9), (P3, 11)] {
                for (trc, t) in [
                    (C::sRGB, 13),
                    (C::Rec709, 1),
                    (C::Linear, 8),
                    (C::PQ, 16),
                    (C::Smpte428, 17),
                    (C::HLG, 18),
                ] {
                    let want = ColorEncoding {
                        white: Named::Value(1),
                        primaries: Named::Value(p),
                        transfer: Transfer::Value(t),
                    };
                    let got = header(colorspace, trc, Some(primaries(named, D65)));
                    let want = (want != ColorEncoding::SRGB).then_some((true, want));
                    assert_eq!(got, want, "{trc:?} {p}");
                }
            }
        }
    }

    #[test]
    fn a_png_s_rounded_chromaticities_are_named() {
        // cHRM's five decimals of BT.2020's and DCI-P3's.
        let bt2020 = [(0.70800, 0.29200), (0.17000, 0.79700), (0.13100, 0.04600)];
        let got = header(
            ColorSpace::RGB,
            ColorCharacteristics::PQ,
            Some(primaries(bt2020, (0.31270, 0.32900))),
        );
        assert_eq!(got.unwrap().1.primaries, Named::Value(9));
        let got = header(
            ColorSpace::RGB,
            ColorCharacteristics::sRGB,
            Some(primaries(P3, DCI)),
        );
        assert_eq!(
            got.unwrap().1,
            ColorEncoding {
                white: Named::Value(11),
                primaries: Named::Value(11),
                transfer: Transfer::Value(13)
            }
        );
        let got = header(
            ColorSpace::RGB,
            ColorCharacteristics::sRGB,
            Some(primaries(SRGB, E)),
        );
        assert_eq!(got.unwrap().1.white, Named::Value(10));
    }

    #[test]
    fn other_primaries_and_white_points_are_written_as_chromaticities() {
        // Adobe RGB (1998)'s primaries, with D50 white.
        let adobe = [(0.64, 0.33), (0.21, 0.71), (0.15, 0.06)];
        let d50 = (0.3457, 0.3585);
        let got = header(
            ColorSpace::RGBA,
            ColorCharacteristics::Gamma2p2,
            Some(primaries(adobe, d50)),
        );
        assert_eq!(
            got,
            Some((
                true,
                ColorEncoding {
                    white: Named::Custom((345_700, 358_500)),
                    primaries: Named::Custom([
                        (640_000, 330_000),
                        (210_000, 710_000),
                        (150_000, 60_000)
                    ]),
                    transfer: Transfer::Gamma(4_545_455)
                }
            ))
        );
    }

    #[test]
    fn a_chromaticity_past_each_range_and_below_zero_reads_back() {
        // Each of the U32's four ranges, and negative values, which
        // imaginary primaries such as ACES's have.
        let wide = [(0.7347, 0.2653), (0.0, 1.0), (0.0001, -0.0770)];
        let far = [(1.2, 0.33), (-0.5, 2.0), (0.15, -1.1)];
        for rgb in [wide, far] {
            let got = header(
                ColorSpace::RGB,
                ColorCharacteristics::Linear,
                Some(primaries(rgb, D65)),
            );
            assert_eq!(
                got.unwrap().1.primaries,
                Named::Custom(rgb.map(millionths)),
                "{rgb:?}"
            );
        }
    }

    #[test]
    fn a_chromaticity_past_what_the_field_holds_is_refused() {
        let data = alloc::vec![100_u8; 12];
        for (rgb, white) in [
            ([(0.64, 0.33), (0.30, 2.1), (0.15, 0.06)], D65),
            (SRGB, (-2.2, 0.33)),
        ] {
            let options = EncoderOptions::new(2, 2, ColorSpace::RGB, BitDepth::Eight);
            let mut encoder = JxlSimpleEncoder::new(&data, options);
            encoder.set_color_primaries(primaries(rgb, white));
            let mut out = Vec::new();
            assert!(
                matches!(encoder.encode(&mut out), Err(JxlEncodeErrors::Generic(_))),
                "{rgb:?} {white:?}"
            );
        }
    }

    #[test]
    fn grayscale_names_its_white_point_and_transfer_and_no_primaries() {
        for (trc, transfer) in [
            (ColorCharacteristics::sRGB, Transfer::Value(13)),
            (ColorCharacteristics::Rec709, Transfer::Value(1)),
            (ColorCharacteristics::Gamma2p8, Transfer::Gamma(3_571_429)),
        ] {
            for colorspace in [ColorSpace::Luma, ColorSpace::LumaA] {
                let got = header(colorspace, trc, Some(primaries(P3, DCI)));
                let want = ColorEncoding {
                    white: Named::Value(11),
                    primaries: Named::Value(1),
                    transfer,
                };
                assert_eq!(got, Some((false, want)), "{trc:?}");
                assert_eq!(
                    header(colorspace, trc, None),
                    Some((
                        false,
                        ColorEncoding {
                            transfer,
                            ..ColorEncoding::SRGB
                        }
                    ))
                );
            }
        }
    }

    #[test]
    fn a_transfer_jpeg_xl_cannot_name_is_refused() {
        use ColorCharacteristics as C;
        for trc in [
            C::Log100,
            C::Log100Sqrt10,
            C::Bt1361,
            C::Smpte240,
            C::Iec61966,
            C::Unknown(2),
        ] {
            let data = alloc::vec![100_u8; 12];
            let options = EncoderOptions::new(2, 2, ColorSpace::RGB, BitDepth::Eight);
            let mut encoder = JxlSimpleEncoder::new(&data, options);
            encoder.set_color_characteristics(trc);
            let mut out = Vec::new();
            assert!(
                matches!(encoder.encode(&mut out), Err(JxlEncodeErrors::Generic(_))),
                "{trc:?}"
            );
        }
    }

    #[test]
    fn an_enum_value_is_coded_by_its_range() {
        assert_eq!(enum_bits(0), [(2, 0), (0, 0)]);
        assert_eq!(enum_bits(1), [(2, 1), (0, 0)]);
        assert_eq!(enum_bits(13), [(2, 2), (4, 11)]);
        assert_eq!(enum_bits(17), [(2, 2), (4, 15)]);
        assert_eq!(enum_bits(18), [(2, 3), (6, 0)]);
    }
}
