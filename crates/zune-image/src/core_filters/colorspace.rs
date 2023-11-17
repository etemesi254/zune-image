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
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::colorspace::{ColorSpace, ALL_COLORSPACES};
use zune_core::log::warn;

use crate::channel::Channel;
use crate::core_filters::colorspace::grayscale::{
    rgb_to_grayscale_f32, rgb_to_grayscale_u16, rgb_to_grayscale_u8
};
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

mod grayscale;
//mod rgb_to_hsl;
mod rgb_to_xyb;

mod rgb_to_cmyk;
pub struct ColorspaceConv {
    to: ColorSpace
}

impl ColorspaceConv {
    pub fn new(to: ColorSpace) -> ColorspaceConv {
        ColorspaceConv { to }
    }
}

fn convert_rgb_to_rgba(image: &mut Image) -> Result<(), ImageErrors> {
    let old_len = image.channels_ref(true)[0].len();

    let bit_type = image.depth().bit_type();

    let new_channel = match bit_type {
        BitType::U8 => {
            let mut channel = Channel::new_with_length::<u8>(old_len);
            channel.fill(255_u8).unwrap();
            channel
        }
        BitType::U16 => {
            let mut channel = Channel::new_with_length::<u16>(old_len);
            channel.fill(65535_u16).unwrap();
            channel
        }
        BitType::F32 => {
            let mut channel = Channel::new_with_length::<f32>(old_len);
            channel.fill(1.0f32).unwrap();
            channel
        }
        _ => {
            return Err(ImageErrors::GenericStr(
                "Unsupported bit depth for RGB->RGBA conversion"
            ))
        }
    };
    if image.is_animated() {
        // multiple images, loop cloning channel
        image
            .frames_mut()
            .iter_mut()
            .for_each(|x| x.push(new_channel.clone()))
    } else {
        // single image, just use the clone we have
        image.frames_mut()[0].push(new_channel);
    }

    Ok(())
}

fn rgb_to_grayscale(
    image: &mut Image, to: ColorSpace, preserve_alpha: bool
) -> Result<(), ImageErrors> {
    let im_colorspace = image.colorspace();

    if im_colorspace == ColorSpace::Luma || im_colorspace == ColorSpace::LumaA {
        warn!("Image already in grayscale skipping this operation");
        return Ok(());
    }

    let (width, height) = image.dimensions();
    let size = width * height * image.depth().size_of();

    let colorspace = image.colorspace();
    let depth = image.depth();
    let max_value = image.depth().max_value();

    let mut out_colorspace = ColorSpace::Unknown;

    for frame in image.frames_mut() {
        let channel = frame.channels_ref(colorspace, preserve_alpha);

        match depth.bit_type() {
            BitType::U8 => {
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

                if preserve_alpha && colorspace.has_alpha() {
                    frame.set_channels(vec![out, channel[3].clone()]);
                    out_colorspace = ColorSpace::LumaA;
                } else if to.has_alpha() {
                    // add alpha channel
                    let mut alpha_out = Channel::new_with_length::<u8>(size);
                    alpha_out.reinterpret_as_mut::<u8>().unwrap().fill(u8::MAX);
                    frame.set_channels(vec![out, alpha_out]);
                    out_colorspace = ColorSpace::Luma;
                } else {
                    frame.set_channels(vec![out]);
                    out_colorspace = ColorSpace::Luma;
                }
            }
            BitType::U16 => {
                let r = channel[0].reinterpret_as::<u16>().unwrap();
                let g = channel[1].reinterpret_as::<u16>().unwrap();
                let b = channel[2].reinterpret_as::<u16>().unwrap();
                let mut out = Channel::new_with_length::<u16>(size);

                rgb_to_grayscale_u16(r, g, b, out.reinterpret_as_mut::<u16>().unwrap(), max_value);

                if preserve_alpha && colorspace.has_alpha() {
                    frame.set_channels(vec![out, channel[3].clone()]);
                    out_colorspace = ColorSpace::LumaA;
                } else if to.has_alpha() {
                    // add alpha channel
                    let mut alpha_out = Channel::new_with_length::<u16>(size);
                    alpha_out
                        .reinterpret_as_mut::<u16>()
                        .unwrap()
                        .fill(u16::MAX);
                    frame.set_channels(vec![out, alpha_out]);
                    out_colorspace = ColorSpace::Luma;
                } else {
                    frame.set_channels(vec![out]);
                    out_colorspace = ColorSpace::Luma;
                }
            }
            BitType::F32 => {
                let r = channel[0].reinterpret_as::<f32>().unwrap();
                let g = channel[1].reinterpret_as::<f32>().unwrap();
                let b = channel[2].reinterpret_as::<f32>().unwrap();
                let mut out = Channel::new_with_length::<f32>(size);

                rgb_to_grayscale_f32(
                    r,
                    g,
                    b,
                    out.reinterpret_as_mut::<f32>().unwrap(),
                    max_value as f32
                );

                if preserve_alpha && colorspace.has_alpha() {
                    frame.set_channels(vec![out, channel[3].clone()]);
                    out_colorspace = ColorSpace::LumaA;
                } else if to.has_alpha() {
                    // add alpha channel
                    let mut alpha_out = Channel::new_with_length::<f32>(size);
                    alpha_out.reinterpret_as_mut::<f32>().unwrap().fill(1.0);
                    frame.set_channels(vec![out, alpha_out]);
                    out_colorspace = ColorSpace::Luma;
                } else {
                    frame.set_channels(vec![out]);
                    out_colorspace = ColorSpace::Luma;
                }
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented("colorspace", d))
        }
    }

    assert_ne!(out_colorspace, ColorSpace::Unknown);

    Ok(())
}

fn convert_rgb_bgr(from: ColorSpace, to: ColorSpace, image: &mut Image) -> Result<(), ImageErrors> {
    for frame in image.frames_mut() {
        // swap B with R
        frame.channels.swap(0, 2);

        // if mapping was from bgra to rgb, drop alpha
        if from == ColorSpace::BGRA && to == ColorSpace::RGB {
            frame.channels.pop();
            assert_eq!(frame.channels.len(), 3);
        }
    }

    // if mapping was from bgra to rgb, drop alpha
    if from == ColorSpace::BGR && to == ColorSpace::RGBA {
        convert_rgb_to_rgba(image)?;
    }
    Ok(())
}

impl OperationsTrait for ColorspaceConv {
    fn name(&self) -> &'static str {
        "Colorspace conversion"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let from = image.colorspace();

        // colorspace matches
        if from == self.to {
            return Ok(());
        }

        let depth = image.depth();
        match (from, self.to) {
            (ColorSpace::RGB, ColorSpace::RGBA) => {
                convert_rgb_to_rgba(image)?;
            }
            (ColorSpace::BGR | ColorSpace::BGRA, ColorSpace::RGB | ColorSpace::RGBA) => {
                convert_rgb_bgr(from, self.to, image)?;
            }
            (ColorSpace::RGB | ColorSpace::RGBA, ColorSpace::BGR | ColorSpace::BGRA) => {
                convert_rgb_bgr(from, self.to, image)?;
            }

            (ColorSpace::RGB | ColorSpace::RGBA, ColorSpace::Luma | ColorSpace::LumaA) => {
                // use the rgb to grayscale converter
                rgb_to_grayscale(image, self.to, self.to.has_alpha())?;
            }
            (ColorSpace::Luma | ColorSpace::LumaA, ColorSpace::RGB | ColorSpace::RGBA) => {
                convert_luma_to_rgb(image, self.to)?;
            }
            (ColorSpace::LumaA, ColorSpace::Luma) => {
                // pop last item in the vec which should
                // contain the alpha channel
                for frame in image.frames_mut() {
                    frame.channels_vec().pop().unwrap();
                }
            }
            (ColorSpace::RGBA, ColorSpace::RGB) => {
                // pop last item in the vec which should
                // contain the alpha channel
                for frame in image.frames_mut() {
                    frame.channels_vec().pop().unwrap();
                }
            }
            (ColorSpace::CMYK, ColorSpace::RGB) => {
                if depth != BitDepth::Eight && depth != BitDepth::Float32 {
                    return Err(ImageErrors::GenericStr(
                        "Can only convert 8 bit and floats from CMYK to RGB"
                    ));
                }
                for frame in image.frames_mut() {
                    let channels = frame.channels_vec();

                    assert_eq!(channels.len(), 4);

                    let (c, rest) = channels.split_at_mut(1);
                    let (m, rest) = rest.split_at_mut(1);
                    let (y, k) = rest.split_at_mut(1);

                    match depth {
                        BitDepth::Eight => {
                            rgb_to_cmyk::cmyk_to_rgb_u8(
                                c[0].reinterpret_as_mut()?,
                                m[0].reinterpret_as_mut()?,
                                y[0].reinterpret_as_mut()?,
                                k[0].reinterpret_as_mut()?
                            );
                        }
                        BitDepth::Float32 => rgb_to_cmyk::cmyk_to_rgb_f32(
                            c[0].reinterpret_as_mut()?,
                            m[0].reinterpret_as_mut()?,
                            y[0].reinterpret_as_mut()?,
                            k[0].reinterpret_as_mut()?
                        ),
                        _ => unreachable!()
                    }

                    // remove K from cymk since the others become RGB
                    channels.pop();
                }
            }

            (ColorSpace::RGB, ColorSpace::CMYK) => {
                if depth != BitDepth::Eight && depth != BitDepth::Float32 {
                    return Err(ImageErrors::GenericStr(
                        "Can only convert 8 bit and float from CYMK  to RGB"
                    ));
                }
                for frame in image.frames_mut() {
                    let channels = frame.channels_vec();

                    assert_eq!(channels.len(), 3);

                    match depth {
                        BitDepth::Eight => {
                            let mut k = Channel::new_with_length::<u8>(channels[0].len());

                            let (r, rest) = channels.split_at_mut(1);
                            let (g, b) = rest.split_at_mut(1);

                            rgb_to_cmyk::rgb_to_cmyk_u8(
                                r[0].reinterpret_as_mut()?,
                                g[0].reinterpret_as_mut()?,
                                b[0].reinterpret_as_mut()?,
                                k.reinterpret_as_mut()?
                            );
                            // remove K from cymk since the others become RGB
                            channels.push(k);
                        }

                        BitDepth::Float32 => {
                            let mut k = Channel::new_with_length::<f32>(channels[0].len());

                            let (r, rest) = channels.split_at_mut(1);
                            let (g, b) = rest.split_at_mut(1);

                            rgb_to_cmyk::rgb_to_cmyk_f32(
                                r[0].reinterpret_as_mut()?,
                                g[0].reinterpret_as_mut()?,
                                b[0].reinterpret_as_mut()?,
                                k.reinterpret_as_mut()?
                            );
                            // remove K from cymk since the others become RGB
                            channels.push(k);
                        }
                        _ => unreachable!()
                    }
                }
            }

            (a, b) => {
                let msg = format!("Unsupported/unknown mapping from {a:?} to {b:?}");
                return Err(ImageErrors::GenericString(msg));
            }
        }
        // set it to the new colorspace
        image.set_colorspace(self.to);

        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &ALL_COLORSPACES
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U16, BitType::U8, BitType::F32]
    }
}

fn convert_luma_to_rgb(image: &mut Image, out_colorspace: ColorSpace) -> Result<(), ImageErrors> {
    let color = image.colorspace();
    for frame in image.frames_mut() {
        let luma_channel = frame.channels_ref(ColorSpace::Luma, true)[0].to_owned();

        if color == ColorSpace::Luma {
            // add two more luma channels
            frame.push(luma_channel.clone());
            frame.push(luma_channel);
        } else if color == ColorSpace::LumaA {
            // we need to insert since layout is
            // Luma, Alpha
            // we want Luma+Luma+Luma+Alpha
            // so we clone and insert
            frame.insert(1, luma_channel.clone());
            frame.insert(1, luma_channel);

            if out_colorspace.has_alpha() {
                // output should not have alpha even if input does
                // we structured it in that alpha channel is the last element
                // in the array, so we can pop it
                frame.channels_vec().pop().expect("No channel present");
            }
        } else {
            let msg = format!(
                "Unsupported colorspace {color:?} in conversion from luma to RGB,colorspace"
            );
            return Err(ImageErrors::GenericString(msg));
        }
    }
    Ok(())
}

#[test]
fn test_cmyk_to_rgb() {
    let mut image = Image::fill(231_u8, ColorSpace::CMYK, 100, 100);
    // just confirm it works and hits the right path
    image.convert_color(ColorSpace::RGB).unwrap();
}

#[test]
fn test_rgb_to_cmyk() {
    let mut image = Image::fill(231_u8, ColorSpace::RGB, 100, 100);
    // just confirm it works and hits the right path
    image.convert_color(ColorSpace::CMYK).unwrap();
}

#[test]
fn test_real_time_rgb_to_cmyk() {
    use zune_core::options::DecoderOptions;
    use zune_jpeg::JpegDecoder;

    use crate::traits::DecoderTrait;

    // checks if conversion for cmyk to rgb holds for jpeg and this routine
    let mut file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // remove /zune-image
    file.pop();
    // remove /crates
    file.pop();
    let actual_file = file.join("test-images/jpeg/cymk.jpg");
    let data = std::fs::read(&actual_file).unwrap();
    // tell jpeg to output to cmyk
    let opts = DecoderOptions::new_fast().jpeg_set_out_colorspace(ColorSpace::CMYK);
    // set it up
    let decoder = JpegDecoder::new_with_options(&data, opts);
    let mut c: Box<dyn DecoderTrait<&Vec<u8>>> = Box::new(decoder);
    let mut im = c.decode().unwrap();
    // just confirm that this is good
    assert_eq!(im.colorspace(), ColorSpace::CMYK);
    // then convert it to rgb
    im.convert_color(ColorSpace::RGB).unwrap();
    // read the same image as rgb
    let new_img = Image::open(&actual_file).unwrap();

    assert!(new_img == im, "RGB to CYMK failed or diverged");
}
