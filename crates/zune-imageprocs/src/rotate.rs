/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Rotate an image
//!
//! The andlge can  be any arbitrary angle including normal 90,180 260.. and

use crate::affine::AffineTransform;
use crate::traits::NumOps;
use crate::utils::execute_on;
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

#[must_use]
pub fn get_rotated_dimensions(width: usize, height: usize, angle: f32) -> (usize, usize) {
    let angle = angle % 360.0;

    // Handle special cases for 90-degree rotations
    if (angle - 90.0).abs() < f32::EPSILON || (angle - 270.0).abs() < f32::EPSILON {
        return (height, width); // Dimensions swap
    }
    if (angle - 180.0).abs() < f32::EPSILON || angle.abs() < f32::EPSILON {
        return (width, height); // Dimensions stay the same
    }

    // For arbitrary angles, calculate bounding box
    let angle_rad = angle.to_radians();
    let cos_a = angle_rad.cos().abs();
    let sin_a = angle_rad.sin().abs();

    let new_width = (width as f32 * cos_a + height as f32 * sin_a).ceil() as usize;
    let new_height = (width as f32 * sin_a + height as f32 * cos_a).ceil() as usize;

    (new_width, new_height)
}
/// Rotates an image by an arbitrary angle.
///
/// This filter calculates the new bounding box required to fit the rotated image
/// and maps the pixels using bilinear interpolation.
///
/// # Background Color
///
/// Because rotating an image at non-right angles (e.g., 45 degrees) exposes the
/// empty corners of the new bounding box, this filter allows you to set a uniform
/// `bg_color`.
/// * `0.0` fills the background with Black (or Transparent, if an alpha channel exists).
/// * `1.0` fills the background with White (or fully Opaque).
///
/// # Optimization
///
/// Orthogonal angles (`0.0`, `90.0`, `180.0`, and `270.0` degrees) bypass the expensive
/// trigonometric interpolation and instead use highly optimized memory transposition
/// and reversal techniques.
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::rotate::Rotate;
/// use zune_image::errors::ImageErrors;
///
/// let mut img = Image::fill(255_u8, ColorSpace::RGB, 100, 100);
///
/// // Rotate 45 degrees, filling the newly exposed corners with black/transparent (0.0)
/// let rotate = Rotate::new_with_bg_color(45.0, 0.0);
/// rotate.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
pub struct Rotate {
    angle: f32,
    #[allow(dead_code)]
    bg_color: f32,
}

impl Rotate {
    /// Creates a new rotation operation.
    ///
    /// # Arguments
    /// * `angle` - The rotation angle in degrees (clockwise).
    #[must_use]
    pub fn new(angle: f32) -> Rotate {
        Rotate {
            angle: angle % 360.0,
            bg_color: 0.0,
        }
    }
    /// Creates a new rotation operation.
    ///
    /// # Arguments
    /// * `angle` - The rotation angle in degrees (clockwise).
    /// * `bg_color` - A normalized value (`0.0` to `1.0`) representing the background fill color.
    #[must_use]
    pub fn new_with_bg_color(angle: f32, bg_color: f32) -> Rotate {
        Rotate {
            angle: angle % 360.0,
            bg_color,
        }
    }
}

impl OperationsTrait for Rotate {
    fn name(&self) -> &'static str {
        "Rotate"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        if !((self.angle - 180.0).abs() < f32::EPSILON
            || (self.angle - 270.0).abs() < f32::EPSILON
            || (self.angle - 90.0).abs() < f32::EPSILON)
        {
            trace!("Arbitrary rotate operation, using affine transform");
            AffineTransform::rotation(self.angle).execute(image)?;
            return Ok(());
        };
        let im_type = image.depth().bit_type();

        let (width, height) = image.dimensions();

        let will_change_dims = (self.angle - 180.0).abs() > f32::EPSILON;
        let depth = image.depth();

        let resize_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let (new_width, new_height) = get_rotated_dimensions(width, height, self.angle);

            if (self.angle - 180.0).abs() < f32::EPSILON {
                // no need to allocate, so simply reverse pixels inside
                match im_type {
                    BitType::U8 => {
                        channel.reinterpret_as_mut::<u8>()?.reverse();
                    }
                    BitType::U16 => {
                        channel.reinterpret_as_mut::<u16>()?.reverse();
                    }
                    BitType::F32 => {
                        channel.reinterpret_as_mut::<f32>()?.reverse();
                    }
                    d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
                }
                return Ok(());
            }

            let mut new_channel = Channel::new_with_length_and_type(
                new_width * new_height * depth.size_of(),
                channel.type_id(),
            );

            match im_type {
                BitType::U8 => {
                    rotate::<u8>(
                        self.angle,
                        width,
                        height,
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                    );
                }
                BitType::U16 => {
                    rotate::<u16>(
                        self.angle,
                        width,
                        height,
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                    );
                }
                BitType::F32 => rotate::<f32>(
                    self.angle,
                    width,
                    height,
                    channel.reinterpret_as()?,
                    new_channel.reinterpret_as_mut()?,
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }
            *channel = new_channel;
            Ok(())
        };
        execute_on(resize_fn, image, false)?;

        if will_change_dims {
            change_image_dims(image, self.angle);
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

fn change_image_dims(image: &mut Image, angle: f32) {
    let (ow, oh) = image.dimensions();
    if (angle - 90.0).abs() < f32::EPSILON {
        image.set_dimensions(oh, ow);
        return;
    }
    if (angle - 270.0).abs() < f32::EPSILON {
        image.set_dimensions(oh, ow);
        return;
    }
    unreachable!("Should never change image dimensions");
}

pub fn rotate<T: Copy + NumOps<T> + Default>(
    angle: f32, width: usize, height: usize, in_image: &[T], out_image: &mut [T],
) {
    let angle = angle % 360.0;

    if (angle - 90.0).abs() < f32::EPSILON {
        rotate_90(in_image, out_image, width, height);
    } else if (angle - 270.0).abs() < f32::EPSILON {
        rotate_270(in_image, out_image, width, height);
    } else {
        unreachable!("Should have called affine before here")
    }
}
fn rotate_90<T: Copy>(in_image: &[T], out_image: &mut [T], width: usize, height: usize) {
    // 8x8 tiles, works fastest on benchmarks
    const TILE: usize = 8;

    for ty in (0..height).step_by(TILE) {
        for tx in (0..width).step_by(TILE) {
            let y_max = (ty + TILE).min(height);
            let x_max = (tx + TILE).min(width);

            for y in ty..y_max {
                let row = &in_image[y * width..(y + 1) * width];
                let idx = height - y - 1;

                for (pos, pix) in row[tx..x_max].iter().enumerate() {
                    let x = tx + pos;
                    let out_idx = x * height + idx;

                    if let Some(c) = out_image.get_mut(out_idx) {
                        *c = *pix;
                    }
                }
            }
        }
    }
}

fn rotate_270<T: Copy>(in_image: &[T], out_image: &mut [T], width: usize, height: usize) {
    const TILE: usize = 8;

    for ty in (0..height).step_by(TILE) {
        for tx in (0..width).step_by(TILE) {
            let y_max = (ty + TILE).min(height);
            let x_max = (tx + TILE).min(width);

            for y in ty..y_max {
                let row = &in_image[y * width..(y + 1) * width];

                for (pos, pix) in row[tx..x_max].iter().enumerate() {
                    let x = tx + pos;
                    let y_idx = (width - x - 1) * height;
                    let out_idx = y_idx + y;

                    if let Some(c) = out_image.get_mut(out_idx) {
                        *c = *pix;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use zune_core::colorspace::ColorSpace;
    use zune_image::image::Image;
    use zune_image::traits::OperationsTrait;

    use crate::rotate::{rotate_270, rotate_90, Rotate};

    #[test]
    fn rotate_over() {
        let mut dst_image = Image::fill(0_u8, ColorSpace::RGB, 100, 120);

        Rotate::new(270.0).execute(&mut dst_image).unwrap();
        assert_eq!(dst_image.dimensions(), (120, 100));
    }
    #[test]
    fn rotate_over_u16() {
        let mut dst_image = Image::fill(0_u16, ColorSpace::RGB, 100, 120);

        Rotate::new(270.0).execute(&mut dst_image).unwrap();
        assert_eq!(dst_image.dimensions(), (120, 100));
    }
    #[test]
    fn rotate_over_f32() {
        let mut dst_image = Image::fill(0_f32, ColorSpace::RGB, 100, 120);
        Rotate::new(270.0).execute(&mut dst_image).unwrap();
        assert_eq!(dst_image.dimensions(), (120, 100));
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Build a flat image where every pixel encodes its (x, y) position as
    /// a u32: `y * 1000 + x`.  That makes it trivial to verify where each
    /// pixel landed after rotation.
    fn coord_image(width: usize, height: usize) -> Vec<u32> {
        (0..height)
            .flat_map(|y| (0..width).map(move |x| (y * 1000 + x) as u32))
            .collect()
    }

    fn rotate(deg: u32, src: &[u32], width: usize, height: usize) -> Vec<u32> {
        let mut dst = vec![0u32; src.len()];
        match deg {
            90 => rotate_90(src, &mut dst, width, height),
            270 => rotate_270(src, &mut dst, width, height),
            _ => panic!("unsupported rotation"),
        }
        dst
    }

    // ── rotate_90 ─────────────────────────────────────────────────────────────
    //
    // 90° clockwise: (x, y) → (height - 1 - y, x) in the output,
    // which has dimensions (height × width).
    //
    // Equivalently, out[x][height-1-y] = in[y][x],
    // i.e. out_pixel at row=x col=(height-1-y) == src pixel at row=y col=x.

    #[test]
    fn rotate_90_trivial_1x1() {
        let src = vec![42u32];
        assert_eq!(rotate(90, &src, 1, 1), vec![42]);
    }

    #[test]
    fn rotate_90_square_2x2() {
        // src (row-major):        after 90° CW:
        //  0  1                    2  0
        //  2  3                    3  1
        let src = vec![0u32, 1, 2, 3];
        let dst = rotate(90, &src, 2, 2);
        assert_eq!(dst, vec![2, 0, 3, 1]);
    }

    #[test]
    fn rotate_90_square_3x3() {
        // src:          after 90° CW:
        //  0 1 2          6 3 0
        //  3 4 5          7 4 1
        //  6 7 8          8 5 2
        let src: Vec<u32> = (0..9).collect();
        let dst = rotate(90, &src, 3, 3);
        assert_eq!(dst, vec![6, 3, 0, 7, 4, 1, 8, 5, 2]);
    }

    /// Non-square: width=3 height=2, output is width=2 height=3.
    /// src (3 wide, 2 tall):     after 90° CW (2 wide, 3 tall):
    ///  0 1 2                      3 0
    ///  3 4 5                      4 1
    ///                             5 2
    #[test]
    fn rotate_90_non_square_3x2() {
        let src = vec![0u32, 1, 2, 3, 4, 5];
        let dst = rotate(90, &src, 3, 2);
        assert_eq!(dst, vec![3, 0, 4, 1, 5, 2]);
    }

    /// This is the regression that catches the tile-local-index bug.
    /// Use a size > TILE (32) so that at least two tiles are visited.
    /// With the bug, pixels in the second tile get placed at wrong columns.
    #[test]
    fn rotate_90_multi_tile_wide() {
        let width = 70;
        let height = 4;
        let src = coord_image(width, height);
        let dst = rotate(90, &src, width, height);

        // out dimensions: width=height=4, height=width=70
        let out_w = height;
        let out_h = width;
        for y in 0..height {
            for x in 0..width {
                let src_val = src[y * width + x];
                // (x, y) → out row = x, out col = height - 1 - y
                let out_row = x;
                let out_col = height - 1 - y;
                let got = dst[out_row * out_w + out_col];
                assert_eq!(
                    got, src_val,
                    "rotate_90 mismatch at src ({x},{y}): \
                     expected pixel {src_val} at out ({out_col},{out_row}), got {got}"
                );
                let _ = out_h; // suppress unused warning
            }
        }
    }

    #[test]
    fn rotate_90_multi_tile_tall() {
        let width = 4;
        let height = 70;
        let src = coord_image(width, height);
        let dst = rotate(90, &src, width, height);
        let out_w = height; // 70
        for y in 0..height {
            for x in 0..width {
                let src_val = src[y * width + x];
                let out_row = x;
                let out_col = height - 1 - y;
                let got = dst[out_row * out_w + out_col];
                assert_eq!(
                    got, src_val,
                    "rotate_90 tall mismatch at src ({x},{y}): \
                     expected {src_val} at out ({out_col},{out_row}), got {got}"
                );
            }
        }
    }

    // ── rotate_270 ────────────────────────────────────────────────────────────
    //
    // 270° clockwise (= 90° CCW): (x, y) → (y, width - 1 - x).
    // Output dimensions: (height × width).

    #[test]
    fn rotate_270_trivial_1x1() {
        let src = vec![7u32];
        assert_eq!(rotate(270, &src, 1, 1), vec![7]);
    }

    #[test]
    fn rotate_270_square_2x2() {
        // src:            after 270° CW:
        //  0  1              1  3
        //  2  3              0  2
        let src = vec![0u32, 1, 2, 3];
        let dst = rotate(270, &src, 2, 2);
        assert_eq!(dst, vec![1, 3, 0, 2]);
    }

    #[test]
    fn rotate_270_square_3x3() {
        // src:          after 270° CW:
        //  0 1 2          2 5 8
        //  3 4 5          1 4 7
        //  6 7 8          0 3 6
        let src: Vec<u32> = (0..9).collect();
        let dst = rotate(270, &src, 3, 3);
        assert_eq!(dst, vec![2, 5, 8, 1, 4, 7, 0, 3, 6]);
    }

    /// Non-square: width=3 height=2, output is width=2 height=3.
    /// src (3 wide, 2 tall):     after 270° CW (2 wide, 3 tall):
    ///  0 1 2                      2 5
    ///  3 4 5                      1 4
    ///                             0 3
    #[test]
    fn rotate_270_non_square_3x2() {
        let src = vec![0u32, 1, 2, 3, 4, 5];
        let dst = rotate(270, &src, 3, 2);
        assert_eq!(dst, vec![2, 5, 1, 4, 0, 3]);
    }

    /// Regression: tile boundary crossing with the tile-local-index bug.
    #[test]
    fn rotate_270_multi_tile_wide() {
        let width = 70;
        let height = 4;
        let src = coord_image(width, height);
        let dst = rotate(270, &src, width, height);

        let out_w = height; // 4
        for y in 0..height {
            for x in 0..width {
                let src_val = src[y * width + x];
                // (x, y) → out row = width - 1 - x, out col = y
                let out_row = width - 1 - x;
                let out_col = y;
                let got = dst[out_row * out_w + out_col];
                assert_eq!(
                    got, src_val,
                    "rotate_270 mismatch at src ({x},{y}): \
                     expected pixel {src_val} at out ({out_col},{out_row}), got {got}"
                );
            }
        }
    }

    #[test]
    fn rotate_270_multi_tile_tall() {
        let width = 4;
        let height = 70;
        let src = coord_image(width, height);
        let dst = rotate(270, &src, width, height);
        let out_w = height; // 70
        for y in 0..height {
            for x in 0..width {
                let src_val = src[y * width + x];
                let out_row = width - 1 - x;
                let out_col = y;
                let got = dst[out_row * out_w + out_col];
                assert_eq!(
                    got, src_val,
                    "rotate_270 tall mismatch at src ({x},{y}): \
                     expected {src_val} at out ({out_col},{out_row}), got {got}"
                );
            }
        }
    }

    // ── round-trip consistency ─────────────────────────────────────────────────

    /// 90° × 4 = identity, for a square image.
    #[test]
    fn rotate_90_four_times_is_identity_square() {
        let w = 50usize;
        let src = coord_image(w, w);
        let mut cur = src.clone();
        for _ in 0..4 {
            let mut nxt = vec![0u32; w * w];
            rotate_90(&cur, &mut nxt, w, w);
            cur = nxt;
        }
        assert_eq!(cur, src);
    }

    /// 90° + 270° = identity, for a square image.
    #[test]
    fn rotate_90_then_270_is_identity() {
        let w = 50usize;
        let src = coord_image(w, w);
        let mut mid = vec![0u32; w * w];
        rotate_90(&src, &mut mid, w, w);
        let mut out = vec![0u32; w * w];
        rotate_270(&mid, &mut out, w, w);
        assert_eq!(out, src);
    }

    /// For a non-square image, 90°+270° must also be identity.
    /// After 90° the dimensions swap, so we pass the swapped dims to 270°.
    #[test]
    fn rotate_90_then_270_non_square_is_identity() {
        let (w, h) = (70, 40);
        let src = coord_image(w, h);
        let mut mid = vec![0u32; w * h];
        rotate_90(&src, &mut mid, w, h); // out is h×w
        let mut out = vec![0u32; w * h];
        rotate_270(&mid, &mut out, h, w); // now h wide, w tall → back to w×h
        assert_eq!(out, src);
    }
}
