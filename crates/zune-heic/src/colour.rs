//! YCbCr → RGB conversion.
//!
//! This follows libheif (1.17) exactly, so decoded pixels are bit-identical
//! to what `heif-convert` / `heif_decode_image` produce:
//!
//! * The colour description of every coded image (a single image, or each
//!   grid tile) comes from that item's `colr` box of type `nclx`, or, when it
//!   has none, from the HEVC VUI, with libde265's defaults for missing fields
//!   (matrix/primaries "unspecified", *limited* range).
//! * Chroma is upsampled nearest-neighbour.
//! * The arithmetic is libheif's single-precision float path, operation for
//!   operation, including its limited-range scale factors and rounding
//!   (`(long)(x + 0.5)`, then clamp).

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;

use crate::header_structs::ColourInformation;
use crate::hevc_decoder::nal_unit_headers::Vui;

/// The parts of an `nclx` colour description that affect YCbCr → RGB.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ColourInfo {
    pub matrix_coefficients: u16,
    pub colour_primaries:    u16,
    pub full_range:          bool
}

impl ColourInfo {
    /// From an item's `colr` property, if it is of type `nclx`.
    pub fn from_colr(colr: &ColourInformation) -> Option<Self> {
        match colr {
            ColourInformation::Nclx {
                colour_primaries,
                matrix_coefficients,
                full_range_flag,
                ..
            } => Some(Self {
                matrix_coefficients: *matrix_coefficients,
                colour_primaries:    *colour_primaries,
                full_range:          *full_range_flag
            }),
            _ => None
        }
    }

    /// From the HEVC VUI. Absent fields take their H.265 defaults, as
    /// libde265 reports them: 2 ("unspecified") for matrix and primaries,
    /// limited range.
    pub fn from_vui(vui: Option<&Vui>) -> Self {
        match vui {
            Some(v) if v.colour_description_present_flag => Self {
                matrix_coefficients: u16::from(v.matrix_coeffs),
                colour_primaries:    u16::from(v.colour_primaries),
                full_range:          v.video_full_range_flag
            },
            Some(v) => Self {
                matrix_coefficients: 2,
                colour_primaries:    2,
                full_range:          v.video_full_range_flag
            },
            None => Self {
                matrix_coefficients: 2,
                colour_primaries:    2,
                full_range:          false
            }
        }
    }
}

/// libheif's `clip_f_u16` for 8-bit output: `(long)(v + 0.5)`, clamped to
/// 0..=255.
#[inline(always)]
fn clip(v: f32) -> u8 {
    ((v + 0.5) as i32).clamp(0, 255) as u8
}

/// libheif's limited-range offset and scale factors.
const LIMITED_OFFSET: f32 = 16.0;
const LIMITED_Y_SCALE: f32 = 1.1689;
const LIMITED_C_SCALE: f32 = 1.1429;

/// A YCbCr → RGB conversion for one colour description.
#[derive(Clone, Debug)]
pub(crate) enum Conversion {
    /// The usual Kr/Kb matrix, as lookup tables.
    Matrix(Arc<MatrixTables>),
    /// matrix_coefficients = 0: the planes hold G, B, R.
    Identity { full: bool },
    /// matrix_coefficients = 8.
    YCgCo
}

/// libheif computes, per sample (in `f32`):
///
/// ```text
/// y'  = y            or (y - 16) * 1.1689   (limited range)
/// cb' = cb - 128     or (cb - 128) * 1.1429
/// cr' = cr - 128     or (cr - 128) * 1.1429
/// R = clip(y' + r_cr * cr')
/// G = clip(y' + g_cb * cb' + g_cr * cr')
/// B = clip(y' + b_cb * cb')
/// ```
///
/// R depends only on (y, cr) and B only on (y, cb), so they are tabulated
/// outright: the table entries are computed with the very same `f32`
/// operations, so they are exact. G depends on all three; its two products
/// are tabulated and the sum is done per sample, in the same order.
#[derive(Debug)]
pub(crate) struct MatrixTables {
    /// `r[(y << 8) | cr]`
    r:    Box<[u8; 65536]>,
    /// `b[(y << 8) | cb]`
    b:    Box<[u8; 65536]>,
    y:    [f32; 256],
    g_cb: [f32; 256],
    g_cr: [f32; 256]
}

impl MatrixTables {
    fn new(r_cr: f32, g_cb: f32, g_cr: f32, b_cb: f32, full: bool) -> Self {
        let y: [f32; 256] = core::array::from_fn(|v| {
            let v = f32::from(v as u8);
            if full { v } else { (v - LIMITED_OFFSET) * LIMITED_Y_SCALE }
        });
        let c: [f32; 256] = core::array::from_fn(|v| {
            let v = f32::from(i16::from(v as u8) - 128);
            if full { v } else { v * LIMITED_C_SCALE }
        });
        let r_cr_c = c.map(|c| r_cr * c);
        let b_cb_c = c.map(|c| b_cb * c);
        let table = || -> Box<[u8; 65536]> { vec![0u8; 65536].into_boxed_slice().try_into().unwrap() };
        let (mut r, mut b) = (table(), table());
        for (yi, &yv) in y.iter().enumerate() {
            for ci in 0..256 {
                r[(yi << 8) | ci] = clip(yv + r_cr_c[ci]);
                b[(yi << 8) | ci] = clip(yv + b_cb_c[ci]);
            }
        }
        Self {
            r,
            b,
            y,
            g_cb: c.map(|c| g_cb * c),
            g_cr: c.map(|c| g_cr * c)
        }
    }

    #[inline(always)]
    fn rgb(&self, y: u8, cb: u8, cr: u8) -> [u8; 3] {
        let (y, cb, cr) = (usize::from(y), usize::from(cb), usize::from(cr));
        [
            self.r[(y << 8) | cr],
            clip(self.y[y] + self.g_cb[cb] + self.g_cr[cr]),
            self.b[(y << 8) | cb]
        ]
    }
}

impl Conversion {
    /// Build the conversion for `info` (for the matrix case this computes
    /// 128 KiB of tables; build it once per colour description).
    pub fn new(info: ColourInfo) -> Self {
        match info.matrix_coefficients {
            0 => Conversion::Identity { full: info.full_range },
            8 => Conversion::YCgCo,
            m => {
                let (r_cr, g_cb, g_cr, b_cb) = match kr_kb(m, info.colour_primaries) {
                    Some((kr, kb)) => (
                        2.0 * (-kr + 1.0),
                        2.0 * kb * (-kb + 1.0) / (kb + kr - 1.0),
                        2.0 * kr * (-kr + 1.0) / (kb + kr - 1.0),
                        2.0 * (-kb + 1.0)
                    ),
                    // libheif's defaults (Rec. 601)
                    None => (1.402, -0.344_136, -0.714_136, 1.772)
                };
                Conversion::Matrix(Arc::new(MatrixTables::new(
                    r_cr,
                    g_cb,
                    g_cr,
                    b_cb,
                    info.full_range
                )))
            }
        }
    }

    /// Convert one sample.
    #[inline(always)]
    pub fn rgb(&self, y: u8, cb: u8, cr: u8) -> [u8; 3] {
        match self {
            Conversion::Matrix(tables) => tables.rgb(y, cb, cr),
            Conversion::Identity { full: true } => [cr, y, cb],
            Conversion::Identity { full: false } => {
                let expand = |v: u8, scale: f32| clip((f32::from(v) - LIMITED_OFFSET) * scale);
                [
                    expand(cr, LIMITED_C_SCALE),
                    expand(y, LIMITED_Y_SCALE),
                    expand(cb, LIMITED_C_SCALE)
                ]
            }
            Conversion::YCgCo => {
                let yv = i32::from(y);
                let cb = i32::from(cb) - 128;
                let cr = i32::from(cr) - 128;
                let c = |v: i32| v.clamp(0, 255) as u8;
                [c(yv - cb + cr), c(yv + cb), c(yv - cb - cr)]
            }
        }
    }

    /// Convert one row: `y_row` luma samples, chroma rows subsampled
    /// horizontally by `sub_x` (1 or 2, nearest-neighbour), into `dst` with
    /// `channels` (3, or 4 with opaque alpha) bytes per pixel.
    pub fn convert_row(
        &self, y_row: &[u8], cb_row: &[u8], cr_row: &[u8], sub_x: usize, dst: &mut [u8],
        channels: usize
    ) {
        match (self, channels) {
            (Conversion::Matrix(t), 3) => {
                convert_row_with::<3>(y_row, cb_row, cr_row, sub_x, dst, |y, cb, cr| t.rgb(y, cb, cr));
            }
            (Conversion::Matrix(t), _) => {
                convert_row_with::<4>(y_row, cb_row, cr_row, sub_x, dst, |y, cb, cr| t.rgb(y, cb, cr));
            }
            (_, 3) => convert_row_with::<3>(y_row, cb_row, cr_row, sub_x, dst, |y, cb, cr| {
                self.rgb(y, cb, cr)
            }),
            _ => convert_row_with::<4>(y_row, cb_row, cr_row, sub_x, dst, |y, cb, cr| {
                self.rgb(y, cb, cr)
            })
        }
    }
}

/// `C` = 3 (RGB) or 4 (RGB + opaque alpha, as alpha is not decoded yet).
#[inline(always)]
fn convert_row_with<const C: usize>(
    y_row: &[u8], cb_row: &[u8], cr_row: &[u8], sub_x: usize, dst: &mut [u8],
    rgb: impl Fn(u8, u8, u8) -> [u8; 3]
) {
    let write = |px: &mut [u8], v: [u8; 3]| {
        px[..3].copy_from_slice(&v);
        if C == 4 {
            px[3] = 255;
        }
    };
    let last_c = cb_row.len() - 1;
    let mut pixels = dst.chunks_exact_mut(C).zip(y_row);
    if sub_x == 2 {
        // pairs of pixels share a chroma sample
        for (&cb, &cr) in cb_row.iter().zip(cr_row) {
            for _ in 0..2 {
                let Some((px, &y)) = pixels.next() else { return };
                write(px, rgb(y, cb, cr));
            }
        }
        // only reached if the chroma row is short
        let (cb, cr) = (cb_row[last_c], cr_row[last_c]);
        for (px, &y) in pixels {
            write(px, rgb(y, cb, cr));
        }
    } else {
        for ((px, &y), (&cb, &cr)) in pixels.zip(cb_row.iter().zip(cr_row)) {
            write(px, rgb(y, cb, cr));
        }
    }
}

/// Kr and Kb for a matrix_coefficients value (libheif's `get_Kr_Kb`), or
/// `None` where libheif falls back to its defaults.
fn kr_kb(matrix: u16, primaries: u16) -> Option<(f32, f32)> {
    let (kr, kb): (f32, f32) = match matrix {
        12 | 13 => {
            let p = colour_primaries(primaries)?;
            let zr = 1.0 - (p.red_x + p.red_y);
            let zg = 1.0 - (p.green_x + p.green_y);
            let zb = 1.0 - (p.blue_x + p.blue_y);
            let zw = 1.0 - (p.white_x + p.white_y);
            let denom = p.white_y
                * (p.red_x * (p.green_y * zb - p.blue_y * zg)
                    + p.green_x * (p.blue_y * zr - p.red_y * zb)
                    + p.blue_x * (p.red_y * zg - p.green_y * zr));
            if denom == 0.0 {
                return None;
            }
            let kr = (p.red_y
                * (p.white_x * (p.green_y * zb - p.blue_y * zg)
                    + p.white_y * (p.blue_x * zg - p.green_x * zb)
                    + zw * (p.green_x * p.blue_y - p.blue_x * p.green_y)))
                / denom;
            let kb = (p.blue_y
                * (p.white_x * (p.red_y * zg - p.green_y * zr)
                    + p.white_y * (p.green_x * zr - p.red_x * zg)
                    + zw * (p.red_x * p.green_y - p.green_x * p.red_y)))
                / denom;
            (kr, kb)
        }
        1 => (0.2126, 0.0722),
        4 => (0.30, 0.11),
        5 | 6 => (0.299, 0.114),
        7 => (0.212, 0.087),
        9 | 10 => (0.2627, 0.0593),
        _ => return None
    };
    // libheif treats Kr = Kb = 0 as "not defined"
    (kr != 0.0 || kb != 0.0).then_some((kr, kb))
}

struct Primaries {
    green_x: f32,
    green_y: f32,
    blue_x:  f32,
    blue_y:  f32,
    red_x:   f32,
    red_y:   f32,
    white_x: f32,
    white_y: f32
}

/// libheif's `get_colour_primaries` (H.273 table 2).
fn colour_primaries(index: u16) -> Option<Primaries> {
    let p = |gx, gy, bx, by, rx, ry, wx, wy| Primaries {
        green_x: gx,
        green_y: gy,
        blue_x:  bx,
        blue_y:  by,
        red_x:   rx,
        red_y:   ry,
        white_x: wx,
        white_y: wy
    };
    Some(match index {
        1 => p(0.300, 0.600, 0.150, 0.060, 0.640, 0.330, 0.3127, 0.3290),
        4 => p(0.21, 0.71, 0.14, 0.08, 0.67, 0.33, 0.310, 0.316),
        5 => p(0.29, 0.60, 0.15, 0.06, 0.64, 0.33, 0.3127, 0.3290),
        6 | 7 => p(0.310, 0.595, 0.155, 0.070, 0.630, 0.340, 0.3127, 0.3290),
        8 => p(0.243, 0.692, 0.145, 0.049, 0.681, 0.319, 0.310, 0.316),
        9 => p(0.170, 0.797, 0.131, 0.046, 0.708, 0.292, 0.3127, 0.3290),
        10 => p(0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.333_333, 0.33333),
        11 => p(0.265, 0.690, 0.150, 0.060, 0.680, 0.320, 0.314, 0.351),
        12 => p(0.265, 0.690, 0.150, 0.060, 0.680, 0.320, 0.3127, 0.3290),
        22 => p(0.295, 0.605, 0.155, 0.077, 0.630, 0.340, 0.3127, 0.3290),
        _ => return None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(matrix: u16, full: bool) -> Conversion {
        Conversion::new(ColourInfo {
            matrix_coefficients: matrix,
            colour_primaries:    2,
            full_range:          full
        })
    }

    #[test]
    fn grey_is_grey() {
        // full range: Y passes through, chroma 128 adds nothing
        for y in [0u8, 1, 77, 128, 254, 255] {
            assert_eq!(conv(2, true).rgb(y, 128, 128), [y; 3]);
            assert_eq!(conv(6, true).rgb(y, 128, 128), [y; 3]);
        }
        // limited range: 16 -> 0, 235 -> 255 (1.1689 * 219 = 255.99, clamped)
        assert_eq!(conv(2, false).rgb(16, 128, 128), [0; 3]);
        assert_eq!(conv(2, false).rgb(235, 128, 128), [255; 3]);
    }

    /// libheif's float path, written out directly
    fn libheif(y: u8, cb: u8, cr: u8, c: (f32, f32, f32, f32), full: bool) -> [u8; 3] {
        let mut yv = f32::from(y);
        let mut cb = f32::from(i16::from(cb) - 128);
        let mut cr = f32::from(i16::from(cr) - 128);
        if !full {
            yv = (yv - 16.0) * 1.1689;
            cb *= 1.1429;
            cr *= 1.1429;
        }
        let clip = |v: f32| ((v + 0.5) as i64).clamp(0, 255) as u8;
        [clip(yv + c.0 * cr), clip(yv + c.1 * cb + c.2 * cr), clip(yv + c.3 * cb)]
    }

    #[test]
    fn tables_match_libheif_formula() {
        // matrix 2: libheif's hard-coded defaults; 1: Rec. 709 from Kr/Kb
        let bt709 = {
            let (kr, kb) = (0.2126f32, 0.0722f32);
            (
                2.0 * (-kr + 1.0),
                2.0 * kb * (-kb + 1.0) / (kb + kr - 1.0),
                2.0 * kr * (-kr + 1.0) / (kb + kr - 1.0),
                2.0 * (-kb + 1.0)
            )
        };
        for (matrix, coeffs) in [(2, (1.402, -0.344_136, -0.714_136, 1.772)), (1, bt709)] {
            for full in [true, false] {
                let conv = conv(matrix, full);
                for y in (0..=255u8).step_by(3) {
                    for cb in (0..=255u8).step_by(5) {
                        for cr in (0..=255u8).step_by(7) {
                            assert_eq!(
                                conv.rgb(y, cb, cr),
                                libheif(y, cb, cr, coeffs, full),
                                "matrix {matrix} full {full} yuv {y} {cb} {cr}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn vui_defaults_are_limited_range() {
        let info = ColourInfo::from_vui(None);
        assert_eq!(info.matrix_coefficients, 2);
        assert!(!info.full_range);
    }
}
