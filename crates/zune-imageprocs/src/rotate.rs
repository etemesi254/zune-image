/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Rotate an image
//!
//! # WARNING
//! - This only works  for rotating 180 degrees.
//!
//! It doesn't work for other rotate angles, this will be fixed later
//!

use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;
use crate::utils::execute_on;

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

pub struct Rotate {
    angle: f32
}

impl Rotate {
    #[must_use]
    pub fn new(angle: f32) -> Rotate {
        Rotate { angle }
    }
}

impl OperationsTrait for Rotate {
    fn name(&self) -> &'static str {
        "Rotate"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let im_type = image.depth().bit_type();

        let (width, height) = image.dimensions();

        let will_change_dims = (self.angle - 180.0).abs() > f32::EPSILON;

        let resize_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let (new_width, new_height) = get_rotated_dimensions(width, height, self.angle);

            let mut new_channel =
                Channel::new_with_length_and_type(new_width * new_height, channel.type_id());

            match im_type {
                BitType::U8 => {
                    rotate::<u8>(
                        self.angle,
                        width,
                        height,
                        new_width,
                        new_height,
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?
                    );
                }
                BitType::U16 => {
                    rotate::<u16>(
                        self.angle,
                        width,
                        height,
                        new_width,
                        new_height,
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?
                    );
                }
                BitType::F32 => rotate::<f32>(
                    self.angle,
                    width,
                    height,
                    new_width,
                    new_height,
                    channel.reinterpret_as()?,
                    new_channel.reinterpret_as_mut()?
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            };
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
    }
    if (angle - 270.0).abs() < f32::EPSILON {
        image.set_dimensions(oh, ow);
    }
    // For arbitrary angles, calculate bounding box
    let angle_rad = angle.to_radians();
    let cos_a = angle_rad.cos().abs();
    let sin_a = angle_rad.sin().abs();

    let new_width = (ow as f32 * cos_a + oh as f32 * sin_a).ceil() as usize;
    let new_height = (ow as f32 * sin_a + oh as f32 * cos_a).ceil() as usize;

    image.set_dimensions(new_width, new_height)
}

pub fn rotate<T: Copy + NumOps<T> + Default>(
    angle: f32, width: usize, height: usize, out_width: usize, out_height: usize, in_image: &[T],
    out_image: &mut [T]
) {
    let angle = angle % 360.0;

    if (angle - 180.0).abs() < f32::EPSILON {
        // copy in image to out image
        out_image.copy_from_slice(in_image);
        rotate_180(out_image, width);
    } else if (angle - 90.0).abs() < f32::EPSILON {
        rotate_90(in_image, out_image, width, height);
    } else if (angle - 270.0).abs() < f32::EPSILON {
        rotate_270(in_image, out_image, width, height);
    } else {
        rotate_arbitrary(
            in_image, out_image, width, height, out_width, out_height, angle
        )
    }
}

fn rotate_180<T: Copy>(in_out_image: &mut [T], width: usize) {
    let half = in_out_image.len() / 2;
    let (top, bottom) = in_out_image.split_at_mut(half);

    for (top_chunk, bottom_chunk) in top
        .chunks_exact_mut(width)
        .zip(bottom.chunks_exact_mut(width).rev())
    {
        for (a, b) in top_chunk.iter_mut().zip(bottom_chunk.iter_mut()) {
            core::mem::swap(a, b);
        }
    }
}

fn rotate_arbitrary<T: Copy + Default + NumOps<T>>(
    in_image: &[T], out_image: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, angle: f32
) {
    let angle_rad = angle.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // Centers
    let in_cx = in_width as f32 / 2.0;
    let in_cy = in_height as f32 / 2.0;
    let out_cx = out_width as f32 / 2.0;
    let out_cy = out_height as f32 / 2.0;

    out_image.fill(T::max_val());

    for out_y in 0..out_height {
        for out_x in 0..out_width {
            // Translate output position to centered coordinates
            let x = out_x as f32 - out_cx;
            let y = out_y as f32 - out_cy;

            // Rotate backwards to find source position
            let src_x = x * cos_a + y * sin_a + in_cx;
            let src_y = -x * sin_a + y * cos_a + in_cy;

            // Check bounds and interpolate
            if src_x >= 0.0
                && src_x < (in_width - 1) as f32
                && src_y >= 0.0
                && src_y < (in_height - 1) as f32
            {
                let x0 = src_x.floor() as usize;
                let y0 = src_y.floor() as usize;
                let x1 = x0 + 1;
                let y1 = y0 + 1;

                let fx = src_x - x0 as f32;
                let fy = src_y - y0 as f32;

                let p00 = in_image[y0 * in_width + x0].to_f32();
                let p10 = in_image[y0 * in_width + x1].to_f32();
                let p01 = in_image[y1 * in_width + x0].to_f32();
                let p11 = in_image[y1 * in_width + x1].to_f32();

                let result = p00 * (1.0 - fx) * (1.0 - fy)
                    + p10 * fx * (1.0 - fy)
                    + p01 * (1.0 - fx) * fy
                    + p11 * fx * fy;

                out_image[out_y * out_width + out_x] = T::from_f32(result);
            }
        }
    }
}
fn rotate_90<T: Copy>(in_image: &[T], out_image: &mut [T], width: usize, height: usize) {
    // TODO: [cae]: Use loop tiling.
    // Does not matter that it is already good enough, we need it fast.
    for (y, pixels) in in_image.chunks_exact(width).enumerate() {
        let idx = height - y - 1;

        for (x, pix) in pixels.iter().enumerate() {
            if let Some(c) = out_image.get_mut((x * height) + idx) {
                *c = *pix;
            }
        }
    }
}

fn rotate_270<T: Copy>(in_image: &[T], out_image: &mut [T], width: usize, height: usize) {
    for (y, pixels) in in_image.chunks_exact(width).enumerate() {
        for (x, pix) in pixels.iter().enumerate() {
            let y_idx = (width - x - 1) * height;
            if let Some(c) = out_image.get_mut(y_idx + y) {
                *c = *pix;
            }
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use zune_image::image::Image;
//     use zune_image::traits::OperationsTrait;
//
//     use crate::rotate::Rotate;
//
//     #[test]
//     fn rotate_over() {
//         let mut dst_image = Image::open("/home/caleb/Pictures/ANIME/418724.png").unwrap();
//         println!("{:?}", dst_image.dimensions());
//
//         Rotate::new(270.0).execute(&mut dst_image).unwrap();
//
//         println!("{:?}", dst_image.dimensions());
//         dst_image.save("./composite.jpg").unwrap();
//     }
// }
