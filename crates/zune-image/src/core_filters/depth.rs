/*
* Copyright (c) 2023.
*
* This software is free software;

 You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
*/
//! Depth conversion routines
//!
//! This helps to convert from one bit depth to another
//!
//! zune-image associates bit depths with native representations
//!, the following mapping indicates the types and range
//!
//!|BitDepth         |native type    |range      |
//!|-----------------|---------------|-----------|
//!|BitDepth::Eight  | [`u8`]        |0   - 255  |
//!|BitDepth::Sixteen| [`u16`]       |0   - 65535|
//!|BitDepth::F32    | [`f32`]       |0.0 - 1.0  |
//!  
//!
//! Conversions are supported from any depth to another, both
//! from and to a depth.
//!
//! The library automatically rescales the pixels during conversion, i.e
//! when moving from `BitDepth::Eight` to `BitDepth::F32`, the library will automatically
//! divide all pixels by `255.0` after converting them to f32's
//!
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::log::trace;

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Convert an image depth from u16 to u8
///
/// This is a simple division depth rescaling, we simply rescale the image pixels
/// mapping the brightest image pixel (e.g 65535 for 16 bit images) to 255 and darkest to
/// zero, squeezing everything else in between.
///
/// # Arguments
///  - `from`: A reference to pixels in 16 bit format
///  - `to`: A mutable reference to pixels in 8 bit format where we will
/// write our pixels
/// - `max_value`: Maximum value we expect this pixel to store.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub(crate) fn depth_u16_to_u8(from: &[u16], to: &mut [u8], max_value: u16) {
    if max_value == u16::MAX {
        // divide by 257, this clamps it to 0..255
        for (old, new) in from.iter().zip(to.iter_mut()) {
            let new_val = (old / 257) as u8;
            *new = new_val;
        }
    } else {
        //okay do scaling
        let max = 1.0 / f32::from(max_value);
        let scale = 255.0;

        for (old, new) in from.iter().zip(to.iter_mut()) {
            let new_val = ((f32::from(*old) * max) * scale) as u8;
            *new = new_val;
        }
    }
}

/// Convert an image depth from u8 to u16
///
/// This is a simple multiplication depth rescaling, we simply rescale the image pixels
/// mapping the brightest image pixel (e.g 255 for 16 bit images) to 65535(16 bit) and darkest to
/// zero, stretching everything else in between.
///
/// # Arguments
///  - `from`: A reference to pixels in 16 bit format
///  - `to`: A mutable reference to pixels in 8 bit format where we will
/// write our pixels
/// - `max_value`: Maximum value we expect this pixel to store.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub(crate) fn depth_u8_to_u16(from: &[u8], to: &mut [u16], max_value: u16) {
    // okay do scaling
    let max = 1.0 / 255.0;
    let scale = f32::from(max_value);

    for (old, new) in from.iter().zip(to.iter_mut()) {
        let new_val = ((f32::from(*old) * max) * scale) as u16;
        *new = new_val;
    }
}

/// Change the image's bit depth from it's initial
/// value to the one specified by this operation.
#[derive(Copy, Clone)]
pub struct Depth {
    depth: BitDepth
}

impl Depth {
    pub fn new(depth: BitDepth) -> Depth {
        Depth { depth }
    }
}

impl OperationsTrait for Depth {
    fn name(&self) -> &'static str {
        "Depth"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let image_depth = image.depth();

        if image_depth == self.depth {
            trace!("Image depth already matches requested, no-op");
            return Ok(());
        }

        for channel in image.channels_mut(false) {
            match (image_depth, self.depth) {
                (BitDepth::Eight, BitDepth::Sixteen) => {
                    let old_data = channel.reinterpret_as().unwrap();
                    let mut new_channel = Channel::new_with_length::<u16>(old_data.len() * 2);

                    let new_channel_raw = new_channel.reinterpret_as_mut().unwrap();

                    depth_u8_to_u16(old_data, new_channel_raw, self.depth.max_value());

                    *channel = new_channel;
                }

                (BitDepth::Sixteen, BitDepth::Eight) => {
                    let old_data = channel.reinterpret_as::<u16>().unwrap();
                    let mut new_channel = Channel::new_with_length::<u8>(channel.len() / 2);

                    let new_channel_raw = new_channel.reinterpret_as_mut().unwrap();

                    depth_u16_to_u8(old_data, new_channel_raw, image_depth.max_value());

                    *channel = new_channel;
                }
                (BitDepth::Float32, BitDepth::Eight) => {
                    let old_data = channel.reinterpret_as::<f32>().unwrap();
                    let mut new_channel = Channel::new_with_length::<u8>(channel.len() / 4);

                    let new_channel_raw = new_channel.reinterpret_as_mut::<u8>().unwrap();

                    // scale by multiplying with 255
                    for (old_chan, new_chan) in old_data.iter().zip(new_channel_raw.iter_mut()) {
                        *new_chan = (255.0 * old_chan).clamp(0.0, 255.0) as u8;
                    }

                    *channel = new_channel;
                }
                (BitDepth::Float32, BitDepth::Sixteen) => {
                    let old_data = channel.reinterpret_as::<f32>().unwrap();
                    let mut new_channel = Channel::new_with_length::<u16>(channel.len() / 2);

                    let new_channel_raw = new_channel.reinterpret_as_mut::<u16>().unwrap();

                    // scale by multiplying with 65535
                    for (old_chan, new_chan) in old_data.iter().zip(new_channel_raw.iter_mut()) {
                        *new_chan = (65535.0 * old_chan).clamp(0.0, 65535.0) as u16;
                    }

                    *channel = new_channel;
                }
                (BitDepth::Eight, BitDepth::Float32) => {
                    let old_data = channel.reinterpret_as::<u8>().unwrap();
                    let mut new_channel = Channel::new_with_length::<f32>(old_data.len() * 4);

                    let new_channel_raw = new_channel.reinterpret_as_mut::<f32>().unwrap();

                    // scale by dividing with 255
                    let recip = 1.0 / 255.0;
                    for (old_chan, new_chan) in old_data.iter().zip(new_channel_raw.iter_mut()) {
                        *new_chan = f32::from(*old_chan) * recip;
                    }

                    *channel = new_channel;
                }
                (BitDepth::Sixteen, BitDepth::Float32) => {
                    let old_data = channel.reinterpret_as::<u16>().unwrap();
                    let mut new_channel = Channel::new_with_length::<f32>(old_data.len() * 4);

                    let new_channel_raw = new_channel.reinterpret_as_mut::<f32>().unwrap();

                    // scale by dividing with 65535
                    let recip = 1.0 / 65535.0;

                    for (old_chan, new_chan) in old_data.iter().zip(new_channel_raw.iter_mut()) {
                        *new_chan = f32::from(*old_chan) * recip;
                    }

                    *channel = new_channel;
                }

                (_, _) => {
                    let msg = format!(
                        "Unknown depth conversion from {:?} to {:?}",
                        image_depth, self.depth
                    );

                    return Err(ImageErrors::GenericString(msg));
                }
            }
        }
        trace!("Image depth changed to {:?}", self.depth);

        image.set_depth(self.depth);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
