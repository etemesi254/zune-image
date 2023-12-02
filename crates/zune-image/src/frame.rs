/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! A single image frame
//!
//! One or more multiple frames make an image
//! an image with multiple frames is considered an animated image

#![allow(dead_code)]

use std::any::TypeId;

use bytemuck::Pod;
use zune_core::colorspace::ColorSpace;

use crate::channel::{Channel, ChannelErrors};
use crate::deinterleave::{deinterleave_f32, deinterleave_u16, deinterleave_u8};
use crate::traits::ZuneInts;

/// A single image frame
///
/// This represents a simple image frame which contains a group
/// of channels whose metadata is contained by the
/// parent image struct.
///
/// Each channel should have the same size
///
/// Each frame also contains a duration or delay, for animated images,
/// this is how long this particular frame should be shown
#[derive(Clone, Eq, PartialEq)]
pub struct Frame {
    pub(crate) channels: Vec<Channel>,
    pub(crate) numerator: usize,
    pub(crate) denominator: usize,
}

impl Frame {
    /// Create a new frame with default duration of 0
    ///
    /// # Arguments
    ///
    /// * `channels`:  Image channels for this frame
    ///
    /// # Examples
    ///
    /// ```
    /// use zune_image::channel::Channel;
    /// use zune_image::frame::Frame;
    /// // create a group of channels, this should
    /// // represent de-interleaved/single channel image contents
    /// let channel = vec![Channel::new::<u8>();3];
    /// // create a new frame  
    /// let frame = Frame::new(channel);
    ///
    /// ```
    pub fn new(channels: Vec<Channel>) -> Frame {
        Frame {
            channels,
            numerator: 1,
            denominator: 1,
        }
    }
    /// Create a new frame from a slice of f32 pixels
    ///
    /// # Arguments
    /// - Colorspace: The colorspace of the pixels
    /// - numerator: Delay numerator
    /// - denominator: Delay denominator
    ///
    /// # Returns
    ///  A new frame
    ///
    /// # Panics
    /// Panics in case the pixels aren't evenly divided by expected number of components on the colorspace
    pub fn from_f32(
        pixels: &[f32], colorspace: ColorSpace, numerator: usize, denominator: usize,
    ) -> Frame {
        let channels = deinterleave_f32(pixels, colorspace).unwrap();

        Frame {
            channels,
            numerator,
            denominator,
        }
    }
    /// Create a new frame from a slice of u16 pixels
    ///
    /// # Arguments
    /// - Colorspace: The colorspace of the pixels
    /// - numerator: Delay numerator
    /// - denominator: Delay denominator
    ///
    /// # Returns
    ///  A new frame
    ///
    /// # Panics
    /// Panics in case the pixels aren't evenly divided by expected number of components on the colorspace

    pub fn from_u16(
        pixels: &[u16], colorspace: ColorSpace, numerator: usize, denominator: usize,
    ) -> Frame {
        let channels = deinterleave_u16(pixels, colorspace).unwrap();
        Frame {
            channels,
            numerator,
            denominator,
        }
    }

    /// Create a new frame from a slice of u8 pixels
    ///
    /// # Arguments
    /// - Colorspace: The colorspace of the pixels
    /// - numerator: Delay numerator
    /// - denominator: Delay denominator
    ///
    /// # Returns
    ///  A new frame
    ///
    /// # Panics
    /// Panics in case the pixels aren't evenly divided by expected number of components on the colorspace

    pub fn from_u8(
        pixels: &[u8], colorspace: ColorSpace, numerator: usize, denominator: usize,
    ) -> Frame {
        let channels = deinterleave_u8(pixels, colorspace).unwrap();
        Frame {
            channels,
            numerator,
            denominator,
        }
    }

    /// Return a mutable reference to the vector of
    /// channels for this frame
    ///
    ///
    /// # Examples
    ///
    ///  Popping the last channel from this frame
    ///
    /// ```
    /// use zune_image::channel::Channel;
    /// use zune_image::frame::Frame;
    ///
    /// let channel = vec![Channel::new::<u8>();4];
    /// let mut frame = Frame::new(channel);
    /// frame.channels_vec().pop();
    ///
    /// // length changed from 4 to 3 since we removed
    /// // the last channel
    /// assert_eq!(frame.channels_vec().len(),3);
    /// ```
    pub fn channels_vec(&mut self) -> &mut Vec<Channel> {
        &mut self.channels
    }

    /// Create a new frame with specified duration
    ///
    /// # Arguments
    ///
    /// * `channels`:  Channels for this frame
    /// * `duration`:  How long we wait for transition of this frame to another frame
    ///
    /// returns: Frame, with the duration
    ///
    /// # Examples
    ///
    /// ```
    /// use zune_image::channel::Channel;
    /// use zune_image::frame::Frame;
    /// let channels = vec![Channel::new::<u8>();3];
    /// // create a new frame
    /// let frame = Frame::new_with_duration(channels,60,1);
    ///
    /// ```
    pub fn new_with_duration(
        channels: Vec<Channel>, numerator: usize, denominator: usize,
    ) -> Frame {
        Frame {
            channels,
            numerator,
            denominator,
        }
    }

    /// Returns a reference to the channels in this frame
    ///
    /// # Arguments
    ///
    /// * `colorspace`:  The colorspace of the  frame, this is gotten from the image metadata
    /// that contains this frame
    ///
    /// * `ignore_alpha`: Whether to ignore the alpha channel.
    ///    If the colorspace has an alpha component, the last channel
    ///     will be ignored as it is assumed to be the alpha channel
    ///
    /// returns: `&[Channel]`: References to the channels
    ///
    /// Also see [get_channels_mut](Self::channels_mut) which returns a
    /// mutable reference to the channels
    pub fn channels_ref(&self, colorspace: ColorSpace, ignore_alpha: bool) -> &[Channel] {
        // check if alpha channel is present in colorspace
        if ignore_alpha && colorspace.has_alpha() {
            let alpha_position = colorspace.alpha_position().unwrap();
            if alpha_position == 0 {
                // cover ARGB
                &self.channels[1..]
            } else if colorspace.alpha_position().unwrap() == colorspace.num_components() - 1 {
                &self.channels[0..colorspace.num_components() - 1]
            } else {
                unreachable!("Should not be here")
            }
        } else {
            &self.channels[0..colorspace.num_components()]
        }
    }
    /// Return a  mutable reference to the underlying channels
    /// # Arguments
    ///
    /// * `colorspace`:  The colorspace of the  frame, this is gotten from the image metadata
    /// that contains this frame
    ///
    /// * `ignore_alpha`: Whether to ignore the alpha channel.
    ///    If the colorspace has an alpha component, the last channel
    ///     will be ignored as it is assumed to be the alpha channel
    ///
    /// returns: `&[Channel]`: References to the channels
    ///
    pub fn channels_mut(&mut self, colorspace: ColorSpace, ignore_alpha: bool) -> &mut [Channel] {
        // check if alpha channel is present in colorspace
        if ignore_alpha && colorspace.has_alpha() {
            // do not take the last one,
            // we assume the last one contains the alpha channel
            // in it.
            // TODO: Is this a bad assumption
            &mut self.channels[0..colorspace.num_components() - 1]
        } else {
            &mut self.channels[0..colorspace.num_components()]
        }
    }
    /// Push a new channel to the end of the channels
    ///
    /// # Arguments
    /// - channel: The channel to be pushed. The length should be equal to other channels length
    ///
    pub fn push(&mut self, channel: Channel) {
        self.channels.push(channel)
    }

    /// Insert a channel into the specified
    /// index
    ///
    /// # Arguments
    ///
    /// * `index`: The index to which we are inserting the
    /// channel
    /// * `channel`: The channel to insert at that specific index
    ///
    pub fn insert(&mut self, index: usize, channel: Channel) {
        self.channels.insert(index, channel)
    }

    /// Write the colorspace to be a four component colorspace
    /// preferably RGBA format
    ///
    /// The following are expected.
    ///
    ///| Component size| Expected colorspace|
    ///|---------------|--------------------|
    ///| 1             | Luma               |
    ///| 2             | LumaA              |
    ///| 3             | RGB                |
    ///| 4             | RGBA               |
    ///
    ///
    /// # Arguments
    ///
    /// * `colorspace`:  Colorspace the frame is in, should be set
    /// by the parent image that owns this frame
    /// * `out_pixel`: The output where we are going to write pixels  to
    ///
    /// returns: Result<(), ChannelErrors>
    ///
    ///  It's an error if `T` is not the same type as the bytes stored by
    /// the channel
    pub fn write_rgba<T: Clone + Copy + ZuneInts<T> + Default + 'static + Pod>(
        &self, colorspace: ColorSpace, out_pixel: &mut [T],
    ) -> Result<(), ChannelErrors> {
        match colorspace.num_components() {
            1 => {
                let luma_channel = self.channels[0].reinterpret_as::<T>()?;

                for (out, luma) in out_pixel.chunks_exact_mut(4).zip(luma_channel) {
                    out[0] = *luma;
                    out[1] = *luma;
                    out[2] = *luma;
                    out[3] = T::max_value();
                }
            }
            2 => {
                let luma_channel = self.channels[0].reinterpret_as::<T>()?;
                let alpha_channel = self.channels[1].reinterpret_as::<T>()?;

                for ((out, luma), alpha) in out_pixel
                    .chunks_exact_mut(4)
                    .zip(luma_channel)
                    .zip(alpha_channel)
                {
                    out[0] = *luma;
                    out[1] = *luma;
                    out[2] = *luma;
                    out[3] = *alpha;
                }
            }
            3 => {
                let c1 = self.channels[0].reinterpret_as::<T>()?;
                let c2 = self.channels[1].reinterpret_as::<T>()?;
                let c3 = self.channels[2].reinterpret_as::<T>()?;

                for (((out, first), second), third) in
                    out_pixel.chunks_exact_mut(4).zip(c1).zip(c2).zip(c3)
                {
                    out[0] = *first;
                    out[1] = *second;
                    out[2] = *third;
                    out[3] = T::max_value();
                }
            }
            4 => {
                let c1 = self.channels[0].reinterpret_as::<T>()?;
                let c2 = self.channels[1].reinterpret_as::<T>()?;
                let c3 = self.channels[2].reinterpret_as::<T>()?;
                let c4 = self.channels[3].reinterpret_as::<T>()?;

                for ((((out, first), second), third), fourth) in out_pixel
                    .chunks_exact_mut(4)
                    .zip(c1)
                    .zip(c2)
                    .zip(c3)
                    .zip(c4)
                {
                    out[0] = *first;
                    out[1] = *second;
                    out[2] = *third;
                    out[3] = *fourth;
                }
            }
            // panics, all the way down
            _ => unreachable!(),
        }
        Ok(())
    }
    pub fn flatten<T: Clone + Default + 'static + Copy + Pod>(
        &self, colorspace: ColorSpace,
    ) -> Vec<T> {
        let out_pixels = match colorspace.num_components() {
            1 => self.channels[0].reinterpret_as::<T>().unwrap().to_vec(),

            2 => {
                let luma_channel = self.channels[0].reinterpret_as::<T>().unwrap();
                let alpha_channel = self.channels[1].reinterpret_as::<T>().unwrap();

                luma_channel
                    .iter()
                    .zip(alpha_channel)
                    .flat_map(|(x1, x2)| [*x1, *x2])
                    .collect::<Vec<T>>()
            }
            3 => {
                let c1 = self.channels[0].reinterpret_as::<T>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<T>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<T>().unwrap();

                c1.iter()
                    .zip(c2)
                    .zip(c3)
                    .flat_map(|((x1, x2), x3)| [*x1, *x2, *x3])
                    .collect::<Vec<T>>()
            }
            4 => {
                let c1 = self.channels[0].reinterpret_as::<T>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<T>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<T>().unwrap();
                let c4 = self.channels[3].reinterpret_as::<T>().unwrap();

                c1.iter()
                    .zip(c2)
                    .zip(c3)
                    .zip(c4)
                    .flat_map(|(((x1, x2), x3), x4)| [*x1, *x2, *x3, *x4])
                    .collect::<Vec<T>>()
            }
            // panics, all the way down
            _ => unreachable!(),
        };

        out_pixels
    }

    /// convert `u16` channels  to native endian
    ///
    ///  # Arguments
    /// - Colorspace of the image
    ///
    /// # Returns
    ///  - A vector with each two bytes representing a u16 value but
    ///
    /// # Panics
    /// If channel isn't storing the u16 as it's internal  type
    pub fn u16_to_native_endian(&self, colorspace: ColorSpace) -> Vec<u8> {
        // confirm all channels are in u16
        for channel in &self.channels {
            if channel.get_type_id() != TypeId::of::<u16>() {
                panic!("Wrong type ID, expected u16 but got another type");
                // wrong type id, that's an error
                //return Err(ImageErrors::WrongTypeId(channel.get_type_id(), U16_TYPE_ID));
            }
        }
        let length = self.channels[0].len() * colorspace.num_components();

        let mut out_pixel = vec![0_u8; length];

        match colorspace.num_components() {
            // reinterpret as u16 first then native endian
            1 => self.channels[0]
                .reinterpret_as::<u16>()
                .unwrap()
                .iter()
                .zip(out_pixel.chunks_exact_mut(2))
                .for_each(|(x, y)| y.copy_from_slice(&x.to_ne_bytes())),

            2 => {
                let luma_channel = self.channels[0].reinterpret_as::<u16>().unwrap();
                let alpha_channel = self.channels[1].reinterpret_as::<u16>().unwrap();

                for ((out, luma), alpha) in out_pixel
                    .chunks_exact_mut(4)
                    .zip(luma_channel)
                    .zip(alpha_channel)
                {
                    out[0..2].copy_from_slice(&luma.to_ne_bytes());
                    out[2..4].copy_from_slice(&alpha.to_ne_bytes());
                }
            }
            3 => {
                let c1 = self.channels[0].reinterpret_as::<u16>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<u16>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<u16>().unwrap();

                for (((out, first), second), third) in
                    out_pixel.chunks_exact_mut(6).zip(c1).zip(c2).zip(c3)
                {
                    out[0..2].copy_from_slice(&first.to_ne_bytes());
                    out[2..4].copy_from_slice(&second.to_ne_bytes());
                    out[4..6].copy_from_slice(&third.to_ne_bytes());
                }
            }
            4 => {
                let c1 = self.channels[0].reinterpret_as::<u16>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<u16>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<u16>().unwrap();
                let c4 = self.channels[3].reinterpret_as::<u16>().unwrap();

                for ((((out, first), second), third), fourth) in out_pixel
                    .chunks_exact_mut(8)
                    .zip(c1)
                    .zip(c2)
                    .zip(c3)
                    .zip(c4)
                {
                    out[0..2].copy_from_slice(&first.to_ne_bytes());
                    out[2..4].copy_from_slice(&second.to_ne_bytes());
                    out[4..6].copy_from_slice(&third.to_ne_bytes());
                    out[6..8].copy_from_slice(&fourth.to_ne_bytes());
                }
            }
            // panics, all the way down
            _ => unreachable!(),
        }
        out_pixel
    }

    /// convert `u16` channels  to big endian
    ///
    ///  # Arguments
    /// - Colorspace of the image
    ///
    /// # Returns
    ///  - A vector with each two bytes representing a u16 value but
    ///
    /// # Panics
    /// If channel isn't storing the u16 as it's internal  type

    pub fn u16_to_big_endian(&self, colorspace: ColorSpace) -> Vec<u8> {
        // confirm all channels are in u16
        for channel in &self.channels {
            if channel.get_type_id() != TypeId::of::<u16>() {
                panic!("Wrong type ID, expected u16 but got another type");
                // wrong type id, that's an error
                //return Err(ImageErrors::WrongTypeId(channel.get_type_id(), U16_TYPE_ID));
            }
        }
        let length = self.channels[0].len() * colorspace.num_components();

        let mut out_pixel = vec![0_u8; length];

        match colorspace.num_components() {
            // reinterpret as u16 first then native endian
            1 => self.channels[0]
                .reinterpret_as::<u16>()
                .unwrap()
                .iter()
                .zip(out_pixel.chunks_exact_mut(2))
                .for_each(|(x, y)| y.copy_from_slice(&x.to_be_bytes())),

            2 => {
                let luma_channel = self.channels[0].reinterpret_as::<u16>().unwrap();
                let alpha_channel = self.channels[1].reinterpret_as::<u16>().unwrap();

                for ((out, luma), alpha) in out_pixel
                    .chunks_exact_mut(4)
                    .zip(luma_channel)
                    .zip(alpha_channel)
                {
                    out[0..2].copy_from_slice(&luma.to_be_bytes());
                    out[2..4].copy_from_slice(&alpha.to_be_bytes());
                }
            }
            3 => {
                let c1 = self.channels[0].reinterpret_as::<u16>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<u16>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<u16>().unwrap();

                for (((out, first), second), third) in
                    out_pixel.chunks_exact_mut(6).zip(c1).zip(c2).zip(c3)
                {
                    out[0..2].copy_from_slice(&first.to_be_bytes());
                    out[2..4].copy_from_slice(&second.to_be_bytes());
                    out[4..6].copy_from_slice(&third.to_be_bytes());
                }
            }
            4 => {
                let c1 = self.channels[0].reinterpret_as::<u16>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<u16>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<u16>().unwrap();
                let c4 = self.channels[3].reinterpret_as::<u16>().unwrap();

                for ((((out, first), second), third), fourth) in out_pixel
                    .chunks_exact_mut(8)
                    .zip(c1)
                    .zip(c2)
                    .zip(c3)
                    .zip(c4)
                {
                    out[0..2].copy_from_slice(&first.to_be_bytes());
                    out[2..4].copy_from_slice(&second.to_be_bytes());
                    out[4..6].copy_from_slice(&third.to_be_bytes());
                    out[6..8].copy_from_slice(&fourth.to_be_bytes());
                }
            }
            // panics, all the way down
            _ => unreachable!(),
        }
        out_pixel
    }
    /// Overwrite the current image channels with new channels
    ///
    /// # Argument
    /// - channels: Image channels that will overwrite the current ones
    ///
    pub fn set_channels(&mut self, channels: Vec<Channel>) {
        self.channels = channels;
    }
}

#[allow(unused_imports)]
mod tests {
    use zune_core::colorspace::ColorSpace;

    use crate::channel::Channel;
    use crate::frame::Frame;

    #[test]
    fn test_conversion_to_native_endian() {
        // test that native endian conversion works for us

        let mut channel = Channel::new::<u16>();
        channel.push(50000_u16);

        let frame = Frame::new(vec![channel]);
        let frame_data = frame.u16_to_native_endian(ColorSpace::Luma);

        assert_eq!(&frame_data, &[80, 195]);
    }

    #[test]
    fn test_flatten_grayscale_to_rgba() {
        let mut channel = Channel::new::<u8>();
        channel.extend::<u8>(&[10, 20, 20]);

        let mut out = vec![0_u8; channel.len() * 4];

        let frame = Frame::new(vec![channel]);
        frame.write_rgba(ColorSpace::Luma, &mut out).unwrap();
        let reference = [10, 10, 10, 255, 20, 20, 20, 255, 20, 20, 20, 255];
        assert_eq!(&out, &reference);
    }
}
