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
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

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

        #[cfg(feature = "threads")]
        {
            trace!("Running rotate in multithreaded mode");
            std::thread::scope(|s| {
                let mut errors = vec![];
                // blur each channel on a separate thread
                for channel in image.channels_mut(false) {
                    let result = s.spawn(|| {
                        let mut new_channel =
                            Channel::new_with_length_and_type(channel.len(), channel.type_id());

                        match im_type {
                            BitType::U8 => {
                                rotate::<u8>(
                                    self.angle,
                                    width,
                                    height,
                                    channel.reinterpret_as()?,
                                    new_channel.reinterpret_as_mut()?
                                );
                            }
                            BitType::U16 => {
                                rotate::<u16>(
                                    self.angle,
                                    width,
                                    height,
                                    channel.reinterpret_as()?,
                                    new_channel.reinterpret_as_mut()?
                                );
                            }
                            BitType::F32 => rotate::<f32>(
                                self.angle,
                                width,
                                height,
                                channel.reinterpret_as()?,
                                new_channel.reinterpret_as_mut()?
                            ),
                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.name(),
                                    d
                                ))
                            }
                        };
                        *channel = new_channel;
                        Ok(())
                    });
                    errors.push(result);
                }
                errors
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }

        #[cfg(not(feature = "threads"))]
        {
            trace!("Running rotate in single-threaded mode");

            for channel in image.channels_mut(false) {
                let mut new_channel =
                    Channel::new_with_length_and_type(channel.len(), channel.get_type_id());
                match im_type {
                    BitType::U8 => {
                        rotate::<u8>(
                            self.angle,
                            width,
                            height,
                            channel.reinterpret_as()?,
                            new_channel.reinterpret_as_mut()?
                        );
                    }
                    BitType::U16 => {
                        rotate::<u16>(
                            self.angle,
                            width,
                            height,
                            channel.reinterpret_as()?,
                            new_channel.reinterpret_as_mut()?
                        );
                    }
                    BitType::F32 => rotate::<f32>(
                        self.angle,
                        width,
                        height,
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?
                    ),
                    d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
                };
                *channel = new_channel;
            }
        }

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
}

pub fn rotate<T: Copy>(
    angle: f32, width: usize, height: usize, in_image: &[T], out_image: &mut [T]
) {
    let angle = angle % 360.0;

    if (angle - 180.0).abs() < f32::EPSILON {
        // copy in image to out image
        out_image.copy_from_slice(in_image);
        rotate_180(out_image, width);
    }
    if (angle - 90.0).abs() < f32::EPSILON {
        rotate_90(in_image, out_image, width, height);
    }
    if (angle - 270.0).abs() < f32::EPSILON {
        rotate_270(in_image, out_image, width, height);
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

fn rotate_90<T: Copy>(in_image: &[T], out_image: &mut [T], width: usize, height: usize) {
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
