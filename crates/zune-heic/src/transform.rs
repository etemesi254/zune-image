/*
 * Copyright (c) 2026.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Image transformations: clean aperture (`clap`) cropping, rotation (`irot`)
//! and mirroring (`imir`).
//!
//! Like libheif (1.17), they are applied in the order the item lists its
//! properties, to the converted (RGB) pixels.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use crate::errors::HeicErrors;

/// One transformation, with the size of the image it applies to implied by
/// its position in the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transform {
    /// Rotate counter-clockwise by 90, 180 or 270 degrees
    Rotate(u16),
    /// Mirror left/right (`imir` axis 1)
    FlipLeftRight,
    /// Mirror top/bottom (`imir` axis 0)
    FlipTopBottom,
    /// Keep the given rectangle
    Crop(Crop)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crop {
    pub left:   usize,
    pub top:    usize,
    pub width:  usize,
    pub height: usize
}

impl Transform {
    /// Size of the image after this transformation of a `w`×`h` image
    pub fn output_size(&self, (w, h): (usize, usize)) -> (usize, usize) {
        match self {
            Transform::Rotate(90 | 270) => (h, w),
            Transform::Rotate(_) | Transform::FlipLeftRight | Transform::FlipTopBottom => (w, h),
            Transform::Crop(c) => (c.width, c.height)
        }
    }
}

/// Size of a `w`×`h` image after all `transforms`
pub(crate) fn output_size(transforms: &[Transform], size: (usize, usize)) -> (usize, usize) {
    transforms.iter().fold(size, |size, t| t.output_size(size))
}

// ── clean aperture ───────────────────────────────────────────────────────────

/// The values of a `clap` box, as (numerator, denominator)
#[derive(Clone, Copy, Debug)]
pub(crate) struct CleanAperture {
    pub width:             (i32, i32),
    pub height:            (i32, i32),
    pub horizontal_offset: (i32, i32),
    pub vertical_offset:   (i32, i32)
}

impl CleanAperture {
    /// The rectangle to keep from a `w`×`h` image, computed exactly like
    /// libheif 1.17 (`Box_clap::*_rounded` and the clamping in
    /// `decode_image_planar`), rounding quirks included.
    pub fn crop(&self, w: usize, h: usize) -> Result<Crop, HeicErrors> {
        let invalid = || HeicErrors::Generic {
            msg: format!("Invalid clean aperture {self:?} for a {w}x{h} image")
        };
        let w32 = i32::try_from(w).map_err(|_| invalid())?;
        let h32 = i32::try_from(h).map_err(|_| invalid())?;

        let width = Fraction::new32(self.width.0, self.width.1);
        let height = Fraction::new32(self.height.0, self.height.1);
        let h_off = Fraction::new32(self.horizontal_offset.0, self.horizontal_offset.1);
        let v_off = Fraction::new32(self.vertical_offset.0, self.vertical_offset.1);

        // left = horizOff + (w - 1)/2 - (clapWidth - 1)/2, rounded down
        let pc_x = h_off.add(Fraction::new32(w32 - 1, 2));
        let left = pc_x.sub(width.sub_int(1).div_int(2)).round_down().ok_or_else(invalid)?;
        let right = width.sub_int(1).add_int(left).round().ok_or_else(invalid)?;
        // top is rounded to nearest (sic)
        let pc_y = v_off.add(Fraction::new32(h32 - 1, 2));
        let top = pc_y.sub(height.sub_int(1).div_int(2)).round().ok_or_else(invalid)?;
        let bottom = height.sub_int(1).add_int(top).round().ok_or_else(invalid)?;

        let left = left.max(0);
        let top = top.max(0);
        let right = right.min(w32 - 1);
        let bottom = bottom.min(h32 - 1);
        if left > right || top > bottom {
            return Err(invalid());
        }
        // all four are now within the image, so non-negative
        let (left, right, top, bottom) =
            (left as usize, right as usize, top as usize, bottom as usize);
        Ok(Crop {
            left,
            top,
            width: right - left + 1,
            height: bottom - top + 1
        })
    }
}

/// Fractions do not grow beyond this in libheif's `Fraction(int32, int32)`
const MAX_FRACTION_VALUE: i32 = 0x10000;

/// A port of libheif's `Fraction`, whose arithmetic loses precision in
/// specific ways that decide the rounding of the crop rectangle.
#[derive(Clone, Copy, Debug)]
struct Fraction {
    num: i32,
    den: i32
}

impl Fraction {
    /// `Fraction(int32_t, int32_t)`: halves both until they are small
    fn new32(num: i32, den: i32) -> Self {
        let (mut num, mut den) = (num, den);
        while !(-MAX_FRACTION_VALUE..=MAX_FRACTION_VALUE).contains(&den) {
            num /= 2;
            den /= 2;
        }
        while den > 1 && !(-MAX_FRACTION_VALUE..=MAX_FRACTION_VALUE).contains(&num) {
            num /= 2;
            den /= 2;
        }
        Self { num, den }
    }

    /// `Fraction(int64_t, int64_t)`: halves both until they fit an i32
    #[allow(clippy::manual_midpoint)] // the exact C expression
    fn new64(num: i64, den: i64) -> Self {
        let (mut num, mut den) = (num, den);
        let fits = |v: i64| i32::try_from(v).is_ok();
        while !fits(num) || !fits(den) {
            num = (num + if num >= 0 { 1 } else { -1 }) / 2;
            den = (den + if den >= 0 { 1 } else { -1 }) / 2;
        }
        Self {
            num: num as i32,
            den: den as i32
        }
    }

    fn add(self, b: Self) -> Self {
        let (an, ad, bn, bd) = (i64::from(self.num), i64::from(self.den), i64::from(b.num), i64::from(b.den));
        if ad == bd {
            Self::new64(an + bn, ad)
        } else {
            Self::new64(an * bd + bn * ad, ad * bd)
        }
    }

    fn sub(self, b: Self) -> Self {
        let (an, ad, bn, bd) = (i64::from(self.num), i64::from(self.den), i64::from(b.num), i64::from(b.den));
        if ad == bd {
            Self::new64(an - bn, ad)
        } else {
            Self::new64(an * bd - bn * ad, ad * bd)
        }
    }

    fn add_int(self, v: i32) -> Self {
        Self::new64(i64::from(self.num) + i64::from(v) * i64::from(self.den), i64::from(self.den))
    }

    fn sub_int(self, v: i32) -> Self {
        Self::new64(i64::from(self.num) - i64::from(v) * i64::from(self.den), i64::from(self.den))
    }

    fn div_int(self, v: i32) -> Self {
        Self::new64(i64::from(self.num), i64::from(self.den) * i64::from(v))
    }

    /// `numerator / denominator` (truncating, like C)
    fn round_down(self) -> Option<i32> {
        self.num.checked_div(self.den)
    }

    /// `(numerator + denominator / 2) / denominator`
    fn round(self) -> Option<i32> {
        let den = i64::from(self.den);
        let v = (i64::from(self.num) + den / 2).checked_div(den)?;
        i32::try_from(v).ok()
    }
}

// ── transforms on interleaved pixels ────────────────────────────────────────

/// Apply `transforms` to the interleaved `w`×`h` image with `channels` bytes
/// per pixel, writing the result to `output`.
///
/// The image is in `canvas`, or already in `output` when `transforms` has
/// only flips (which work in place; the caller then passes an empty
/// `canvas`). The last rotation or crop writes straight into `output`.
pub(crate) fn apply_interleaved(
    transforms: &[Transform], channels: usize, canvas: Vec<u8>, size: (usize, usize),
    output: &mut [u8]
) -> Result<(), HeicErrors> {
    let last_copy = transforms
        .iter()
        .rposition(|t| matches!(t, Transform::Rotate(_) | Transform::Crop(_)));
    let mut current = canvas;
    let mut in_output = last_copy.is_none();
    let (mut w, mut h) = size;

    for (i, t) in transforms.iter().enumerate() {
        let (nw, nh) = t.output_size((w, h));
        match t {
            Transform::FlipLeftRight | Transform::FlipTopBottom => {
                let buf = if in_output { &mut *output } else { &mut current };
                flip_interleaved(buf, w, h, channels, *t == Transform::FlipLeftRight);
            }
            Transform::Rotate(_) | Transform::Crop(_) => {
                let size = nw * nh * channels;
                if Some(i) == last_copy {
                    let dst = output.get_mut(..size).ok_or(HeicErrors::Generic {
                        msg: "Output buffer too small for the transformed image".into()
                    })?;
                    copy_transformed(t, &current, w, h, channels, dst)?;
                    current = Vec::new();
                    in_output = true;
                } else {
                    let mut next = vec![0u8; size];
                    copy_transformed(t, &current, w, h, channels, &mut next)?;
                    current = next;
                }
            }
        }
        (w, h) = (nw, nh);
    }
    Ok(())
}

/// Rotate or crop `src` (`w`×`h`, `channels` bytes per pixel) into `dst`
fn copy_transformed(
    t: &Transform, src: &[u8], w: usize, h: usize, channels: usize, dst: &mut [u8]
) -> Result<(), HeicErrors> {
    match *t {
        Transform::Rotate(degrees) => rotate_ccw(src, w, h, channels, degrees, dst),
        Transform::Crop(c) => {
            let row = c.width * channels;
            for (y, out) in dst.chunks_exact_mut(row).take(c.height).enumerate() {
                let start = ((c.top + y) * w + c.left) * channels;
                out.copy_from_slice(&src[start..start + row]);
            }
            Ok(())
        }
        _ => Ok(())
    }
}

/// Rotate counter-clockwise (libheif's `rotate_ccw`), `channels` bytes per
/// pixel; `channels` = 1 rotates a single plane.
fn rotate_ccw(
    src: &[u8], w: usize, h: usize, channels: usize, degrees: u16, dst: &mut [u8]
) -> Result<(), HeicErrors> {
    let c = channels;
    match degrees {
        90 => {
            // out(x, y) = in(w - 1 - y, x), output is h wide
            for (y, out) in dst.chunks_exact_mut(h * c).take(w).enumerate() {
                let sx = w - 1 - y;
                for (x, px) in out.chunks_exact_mut(c).enumerate() {
                    let s = (x * w + sx) * c;
                    px.copy_from_slice(&src[s..s + c]);
                }
            }
        }
        180 => {
            for (y, out) in dst.chunks_exact_mut(w * c).take(h).enumerate() {
                let src_row = &src[(h - 1 - y) * w * c..(h - y) * w * c];
                for (px, sp) in out.chunks_exact_mut(c).zip(src_row.chunks_exact(c).rev()) {
                    px.copy_from_slice(sp);
                }
            }
        }
        270 => {
            // out(x, y) = in(y, h - 1 - x), output is h wide
            for (y, out) in dst.chunks_exact_mut(h * c).take(w).enumerate() {
                for (x, px) in out.chunks_exact_mut(c).enumerate() {
                    let s = ((h - 1 - x) * w + y) * c;
                    px.copy_from_slice(&src[s..s + c]);
                }
            }
        }
        _ => {
            return Err(HeicErrors::Generic {
                msg: format!("Unsupported rotation: {degrees}")
            });
        }
    }
    Ok(())
}

/// Mirror in place, left/right or top/bottom
fn flip_interleaved(buf: &mut [u8], w: usize, h: usize, channels: usize, left_right: bool) {
    let row_len = w * channels;
    let buf = &mut buf[..row_len * h];
    if left_right {
        for row in buf.chunks_exact_mut(row_len) {
            for x in 0..w / 2 {
                let (a, b) = (x * channels, (w - 1 - x) * channels);
                for k in 0..channels {
                    row.swap(a + k, b + k);
                }
            }
        }
    } else {
        for y in 0..h / 2 {
            let (top, bottom) = buf.split_at_mut((h - 1 - y) * row_len);
            top[y * row_len..(y + 1) * row_len].swap_with_slice(&mut bottom[..row_len]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clap(w: (i32, i32), h: (i32, i32), ho: (i32, i32), vo: (i32, i32)) -> CleanAperture {
        CleanAperture {
            width:             w,
            height:            h,
            horizontal_offset: ho,
            vertical_offset:   vo
        }
    }

    #[test]
    fn clap_like_libheif_encoder() {
        // what libheif writes for a 301x203 image coded as 302x204
        let c = clap((301, 1), (203, 1), (-1, 2), (-1, 2)).crop(302, 204).unwrap();
        assert_eq!(c, Crop { left: 0, top: 0, width: 301, height: 203 });
    }

    #[test]
    fn clap_centred() {
        let c = clap((100, 1), (50, 1), (0, 1), (0, 1)).crop(200, 100).unwrap();
        assert_eq!(c, Crop { left: 50, top: 25, width: 100, height: 50 });
    }

    #[test]
    fn clap_clamped_and_invalid() {
        // larger than the image: clamped
        let c = clap((400, 1), (300, 1), (0, 1), (0, 1)).crop(200, 100).unwrap();
        assert_eq!((c.left, c.top, c.width, c.height), (0, 0, 200, 100));
        // completely outside
        assert!(clap((10, 1), (10, 1), (1000, 1), (0, 1)).crop(200, 100).is_err());
    }

    #[test]
    fn rotations_and_flips() {
        // 3x2 image, one byte per pixel:
        // 1 2 3
        // 4 5 6
        let src = [1, 2, 3, 4, 5, 6];
        let mut out = [0u8; 6];
        rotate_ccw(&src, 3, 2, 1, 90, &mut out).unwrap();
        assert_eq!(out, [3, 6, 2, 5, 1, 4]);
        rotate_ccw(&src, 3, 2, 1, 270, &mut out).unwrap();
        assert_eq!(out, [4, 1, 5, 2, 6, 3]);
        rotate_ccw(&src, 3, 2, 1, 180, &mut out).unwrap();
        assert_eq!(out, [6, 5, 4, 3, 2, 1]);

        let mut buf = src;
        flip_interleaved(&mut buf, 3, 2, 1, true);
        assert_eq!(buf, [3, 2, 1, 6, 5, 4]);
        let mut buf = src;
        flip_interleaved(&mut buf, 3, 2, 1, false);
        assert_eq!(buf, [4, 5, 6, 1, 2, 3]);
    }

}
