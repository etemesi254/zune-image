/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Colorspace conversion routines
//!
//!
//! This contains simple colorspace conversion routines
//! that convert between different colorspaces in image
//!
use log::warn;
use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;

use crate::channel::Channel;
use crate::core_filters::colorspace::grayscale::{rgb_to_grayscale_u16, rgb_to_grayscale_u8};
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

mod grayscale;
mod rgb_to_xyb;

pub struct ColorspaceConv
{
    to: ColorSpace
}

impl ColorspaceConv
{
    pub fn new(to: ColorSpace) -> ColorspaceConv
    {
        ColorspaceConv { to }
    }
}

fn convert_rgb_to_rgba(image: &mut Image) -> Result<(), ImageErrors>
{
    let old_len = image.get_channels_ref(true)[0].len();

    let bit_type = image.get_depth().bit_type();

    let new_channel = match bit_type
    {
        BitType::U8 =>
        {
            let mut channel = Channel::new_with_length::<u8>(old_len);
            channel.fill(255_u8).unwrap();
            channel
        }
        BitType::U16 =>
        {
            let mut channel = Channel::new_with_length::<u16>(old_len);
            channel.fill(65535_u16).unwrap();
            channel
        }
        BitType::F32 =>
        {
            let mut channel = Channel::new_with_length::<f32>(old_len);
            channel.fill(1.0f32).unwrap();
            channel
        }
        _ =>
        {
            return Err(ImageErrors::GenericStr(
                "Unsupported bit depth for RGB->RGBA conversion"
            ))
        }
    };
    if image.is_animated()
    {
        // multiple images, loop cloning channel
        image
            .get_frames_mut()
            .iter_mut()
            .for_each(|x| x.add(new_channel.clone()))
    }
    else
    {
        // single image, just use the clone we have
        image.get_frames_mut()[0].add(new_channel);
    }

    Ok(())
}

fn rgb_to_grayscale(image: &mut Image, preserve_alpha: bool) -> Result<(), ImageErrors>
{
    let im_colorspace = image.get_colorspace();

    if im_colorspace == ColorSpace::Luma || im_colorspace == ColorSpace::LumaA
    {
        warn!("Image already in grayscale skipping this operation");
        return Ok(());
    }

    let (width, height) = image.get_dimensions();
    let size = width * height * image.get_depth().size_of();

    let colorspace = image.get_colorspace();
    let depth = image.get_depth();
    let max_value = image.get_depth().max_value();

    let mut out_colorspace = ColorSpace::Unknown;

    for frame in image.get_frames_mut()
    {
        let channel = frame.get_channels_ref(colorspace, preserve_alpha);

        match depth.bit_type()
        {
            BitType::U8 =>
            {
                let r = channel[0].reinterpret_as::<u8>().unwrap();
                let g = channel[1].reinterpret_as::<u8>().unwrap();
                let b = channel[2].reinterpret_as::<u8>().unwrap();
                let mut out = Channel::new_with_length::<u8>(size);

                rgb_to_grayscale_u8(
                    r,
                    g,
                    b,
                    out.reinterpret_as_mut::<u8>().unwrap(),
                    max_value as u8
                );

                if preserve_alpha && colorspace.has_alpha()
                {
                    frame.set_channels(vec![out, channel[3].clone()]);
                    out_colorspace = ColorSpace::LumaA;
                }
                else
                {
                    frame.set_channels(vec![out]);
                    out_colorspace = ColorSpace::Luma;
                }
            }
            BitType::U16 =>
            {
                let r = channel[0].reinterpret_as::<u16>().unwrap();
                let g = channel[1].reinterpret_as::<u16>().unwrap();
                let b = channel[2].reinterpret_as::<u16>().unwrap();
                let mut out = Channel::new_with_length::<u16>(size);

                rgb_to_grayscale_u16(r, g, b, out.reinterpret_as_mut::<u16>().unwrap(), max_value);

                if preserve_alpha && colorspace.has_alpha()
                {
                    frame.set_channels(vec![out, channel[3].clone()]);
                    out_colorspace = ColorSpace::LumaA;
                }
                else
                {
                    frame.set_channels(vec![out]);
                    out_colorspace = ColorSpace::Luma;
                }
            }
            _ => todo!()
        }
    }

    assert_ne!(out_colorspace, ColorSpace::Unknown);

    Ok(())
}

fn convert_rgb_bgr(from: ColorSpace, to: ColorSpace, image: &mut Image) -> Result<(), ImageErrors>
{
    for frame in image.get_frames_mut()
    {
        // swap B with R
        frame.channels.swap(0, 2);

        // if mapping was from bgra to rgb, drop alpha
        if from == ColorSpace::BGRA && to == ColorSpace::RGB
        {
            frame.channels.pop();
            assert_eq!(frame.channels.len(), 3);
        }
    }

    // if mapping was from bgra to rgb, drop alpha
    if from == ColorSpace::BGR && to == ColorSpace::RGBA
    {
        convert_rgb_to_rgba(image)?;
    }
    Ok(())
}

impl OperationsTrait for ColorspaceConv
{
    fn get_name(&self) -> &'static str
    {
        "Colorspace conversion"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let from = image.get_colorspace();

        // colorspace matches
        if from == self.to
        {
            return Ok(());
        }

        match (from, self.to)
        {
            (ColorSpace::RGB, ColorSpace::RGBA) =>
            {
                convert_rgb_to_rgba(image)?;
            }
            (ColorSpace::BGR | ColorSpace::BGRA, ColorSpace::RGB | ColorSpace::RGBA) =>
            {
                convert_rgb_bgr(from, self.to, image)?;
            }
            (ColorSpace::RGB | ColorSpace::RGBA, ColorSpace::BGR | ColorSpace::BGRA) =>
            {
                convert_rgb_bgr(from, self.to, image)?;
            }

            (ColorSpace::RGB | ColorSpace::RGBA, ColorSpace::Luma | ColorSpace::LumaA) =>
            {
                // use the rgb to grayscale converter
                rgb_to_grayscale(image, self.to.has_alpha())?;
            }
            (ColorSpace::Luma | ColorSpace::LumaA, ColorSpace::RGB | ColorSpace::RGBA) =>
            {
                convert_luma_to_rgb(image, self.to)?;
            }
            (ColorSpace::LumaA, ColorSpace::Luma) =>
            {
                // pop last item in the vec which should
                // contain the alpha channel
                for frame in image.get_frames_mut()
                {
                    frame.channels_vec().pop().unwrap();
                }
            }
            (ColorSpace::RGBA, ColorSpace::RGB) =>
            {
                // pop last item in the vec which should
                // contain the alpha channel
                for frame in image.get_frames_mut()
                {
                    frame.channels_vec().pop().unwrap();
                }
            }

            (a, b) =>
            {
                let msg = format!("Unsupported/unknown mapping from {a:?} to {b:?}");
                return Err(ImageErrors::GenericString(msg));
            }
        }
        // set it to the new colorspace
        image.set_colorspace(self.to);

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U16, BitType::U8, BitType::F32]
    }
}

fn convert_luma_to_rgb(image: &mut Image, out_colorspace: ColorSpace) -> Result<(), ImageErrors>
{
    let color = image.get_colorspace();
    for frame in image.get_frames_mut()
    {
        let luma_channel = frame.get_channels_ref(ColorSpace::Luma, true)[0].to_owned();

        if color == ColorSpace::Luma
        {
            // add two more luma channels
            frame.add(luma_channel.clone());
            frame.add(luma_channel);
        }
        else if color == ColorSpace::LumaA
        {
            // we need to insert since layout is
            // Luma, Alpha
            // we want Luma+Luma+Luma+Alpha
            // so we clone and insert
            frame.insert(1, luma_channel.clone());
            frame.insert(1, luma_channel);

            if out_colorspace.has_alpha()
            {
                // output should not have alpha even if input does
                // we structured it in that alpha channel is the last element
                // in the array, so we can pop it
                frame.channels_vec().pop().expect("No channel present");
            }
        }
        else
        {
            let msg = format!(
                "Unsupported colorspace {color:?} in conversion from luma to RGB,colorspace"
            );
            return Err(ImageErrors::GenericString(msg));
        }
    }
    Ok(())
}
