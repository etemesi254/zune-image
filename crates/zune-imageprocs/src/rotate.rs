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

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::frame::Frame;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionMut;
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
        if self.angle.abs() < f32::EPSILON {
            // zero rotation
            return Ok(());
        }
        let is_180 = (self.angle - 180.0).abs() < f32::EPSILON;
        let is_90 = (self.angle - 90.0).abs() < f32::EPSILON;
        let is_270 = (self.angle - 270.0).abs() < f32::EPSILON;

        if !is_180 && !is_90 && !is_270 {
            trace!("Arbitrary rotate operation, using affine transform");
            AffineTransform::rotation(self.angle).execute_impl(image)?;
            return Ok(());
        }

        let depth = image.depth().bit_type();
        let colorspace = image.colorspace();

        // --- 180 Degree Fast Path (In-Place Reversal) ---
        if is_180 {
            for frame in image.frames_mut() {
                for channel in frame.channels_mut(colorspace, false) {
                    match depth {
                        BitType::U8 => channel.reinterpret_as_mut::<u8>()?.reverse(),
                        BitType::U16 => channel.reinterpret_as_mut::<u16>()?.reverse(),
                        BitType::F32 => channel.reinterpret_as_mut::<f32>()?.reverse(),
                        d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
                    }
                }
            }
            return Ok(());
        }

        // --- 90 & 270 Degree Multi-Threaded Path ---
        let (old_w, old_h) = image.dimensions();
        let new_w = old_h;
        let new_h = old_w;

        for frame in image.frames_mut() {
            let src_channels: Vec<&Channel> =
                frame.channels_ref(colorspace, false).iter().collect();
            let num_channels = src_channels.len();

            let new_channels = vec![Channel::new_with_bit_type(new_w * new_h, depth); num_channels];
            let mut dest_frame = Frame::new(new_channels);

            match depth {
                BitType::U8 => {
                    let src_slices: Vec<&[u8]> = src_channels
                        .iter()
                        .map(|c| c.reinterpret_as().unwrap())
                        .collect();

                    Image::par_process_frame_regions::<u8, _>(
                        &mut dest_frame,
                        new_w,
                        new_h,
                        colorspace,
                        false,
                        |region| {
                            if is_90 {
                                rotate_90_region(region, &src_slices, old_w, old_h);
                            } else {
                                rotate_270_region(region, &src_slices, old_w, old_h);
                            }
                        },
                    )?;
                }
                BitType::U16 => {
                    let src_slices: Vec<&[u16]> = src_channels
                        .iter()
                        .map(|c| c.reinterpret_as().unwrap())
                        .collect();
                    Image::par_process_frame_regions::<u16, _>(
                        &mut dest_frame,
                        new_w,
                        new_h,
                        colorspace,
                        false,
                        |region| {
                            if is_90 {
                                rotate_90_region(region, &src_slices, old_w, old_h);
                            } else {
                                rotate_270_region(region, &src_slices, old_w, old_h);
                            }
                        },
                    )?;
                }
                BitType::F32 => {
                    let src_slices: Vec<&[f32]> = src_channels
                        .iter()
                        .map(|c| c.reinterpret_as().unwrap())
                        .collect();
                    Image::par_process_frame_regions::<f32, _>(
                        &mut dest_frame,
                        new_w,
                        new_h,
                        colorspace,
                        false,
                        |region| {
                            if is_90 {
                                rotate_90_region(region, &src_slices, old_w, old_h);
                            } else {
                                rotate_270_region(region, &src_slices, old_w, old_h);
                            }
                        },
                    )?;
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }

            for (old_chan, new_chan) in frame
                .channels_mut(colorspace, false)
                .iter_mut()
                .zip(dest_frame.channels_mut(colorspace, false))
            {
                std::mem::swap(old_chan, new_chan);
            }
        }

        image.set_dimensions(new_w, new_h);
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
/// 90° Clockwise Rotation
fn rotate_90_region<T: Copy>(
    region: &mut PlanarRegionMut<'_, T>, src_channels: &[&[T]], old_w: usize, old_h: usize,
) {
    let out_w = region.width;
    let num_channels = region.channels.len();

    // Tiling prevents cache misses on strided vertical reads
    const TILE: usize = 8;

    for ty in (0..region.height).step_by(TILE) {
        let y_max = (ty + TILE).min(region.height);

        for tx in (0..out_w).step_by(TILE) {
            let x_max = (tx + TILE).min(out_w);

            for local_y in ty..y_max {
                let dest_y = region.y_offset + local_y;
                let src_x = dest_y;
                let dest_row_offset = local_y * out_w;

                for dest_x in tx..x_max {
                    let src_y = old_h - 1 - dest_x;

                    let src_idx = src_y * old_w + src_x;
                    let dest_idx = dest_row_offset + dest_x;

                    // Write all channels simultaneously for max cache utilization
                    for c in 0..num_channels {
                        region.channels[c][dest_idx] = src_channels[c][src_idx];
                    }
                }
            }
        }
    }
}

/// 270° Clockwise Rotation (or 90° Counter-Clockwise)
fn rotate_270_region<T: Copy>(
    region: &mut PlanarRegionMut<'_, T>, src_channels: &[&[T]], old_w: usize, _old_h: usize,
) {
    let out_w = region.width;
    let num_channels = region.channels.len();

    const TILE: usize = 8;

    for ty in (0..region.height).step_by(TILE) {
        let y_max = (ty + TILE).min(region.height);

        for tx in (0..out_w).step_by(TILE) {
            let x_max = (tx + TILE).min(out_w);

            for local_y in ty..y_max {
                let dest_y = region.y_offset + local_y;
                let src_x = old_w - 1 - dest_y;
                let dest_row_offset = local_y * out_w;

                for dest_x in tx..x_max {
                    let src_y = dest_x;

                    let src_idx = src_y * old_w + src_x;
                    let dest_idx = dest_row_offset + dest_x;

                    for c in 0..num_channels {
                        region.channels[c][dest_idx] = src_channels[c][src_idx];
                    }
                }
            }
        }
    }
}
