use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::colorspace::ColorSpace;
use zune_core::log::warn;

use crate::channel::Channel;
use crate::core_filters::colorspace::grayscale::{
    rgb_to_grayscale_f32, rgb_to_grayscale_u16, rgb_to_grayscale_u8
};
use crate::core_filters::colorspace::rgb_to_cmyk;
use crate::core_filters::colorspace::rgb_to_hsl::{hsl_to_rgb, rgb_to_hsl};
use crate::core_filters::colorspace::rgb_to_hsv::{hsv_to_rgb, rgb_to_hsv};
use crate::errors::ImageErrors;
use crate::image::Image;

pub fn convert_adding_opaque_alpha(image: &mut Image) -> Result<(), ImageErrors> {
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

pub fn convert_rgb_to_grayscale(
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
        let channel = frame.channels_ref(colorspace, !preserve_alpha);

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

pub fn convert_rgb_bgr(
    from: ColorSpace, to: ColorSpace, image: &mut Image
) -> Result<(), ImageErrors> {
    for frame in image.frames_mut() {
        // swap B with R
        frame.channels.swap(0, 2);

        // if mapping was from alpha to non-alpha drop alpha
        if (from == ColorSpace::BGRA || from == ColorSpace::RGBA)
            && (to == ColorSpace::RGB || to == ColorSpace::BGR)
        {
            frame.channels.pop();
            assert_eq!(frame.channels.len(), 3);
        }
    }

    // if mapping was from non alpha to alpha, add opaque alpha
    if (from == ColorSpace::BGR || from == ColorSpace::RGB)
        && (to == ColorSpace::RGBA || to == ColorSpace::BGRA)
    {
        convert_adding_opaque_alpha(image)?;
    }
    Ok(())
}

pub fn convert_rgb_to_cmyk(image: &mut Image) -> Result<(), ImageErrors> {
    let depth = image.depth();

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
                // add K
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
                // add K
                channels.push(k);
            }
            BitDepth::Sixteen => {
                let mut k = Channel::new_with_length::<u16>(channels[0].len());

                let (r, rest) = channels.split_at_mut(1);
                let (g, b) = rest.split_at_mut(1);

                rgb_to_cmyk::rgb_to_cmyk_u16(
                    r[0].reinterpret_as_mut()?,
                    g[0].reinterpret_as_mut()?,
                    b[0].reinterpret_as_mut()?,
                    k.reinterpret_as_mut()?
                );
                // add K
                channels.push(k);
            }
            _ => unreachable!()
        }
    }
    Ok(())
}
pub fn convert_cmyk_to_rgb(image: &mut Image, to: ColorSpace) -> Result<(), ImageErrors> {
    let depth = image.depth();

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
            BitDepth::Sixteen => rgb_to_cmyk::cmyk_to_rgb_u16(
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
    if to == ColorSpace::RGBA {
        // add opaque alpha channel
        convert_adding_opaque_alpha(image)?;
    }
    Ok(())
}

pub fn convert_rgb_to_hsl(image: &mut Image) -> Result<(), ImageErrors> {
    image.convert_color(ColorSpace::RGB)?; // recursive functions, what could go wrong
                                           // preserve original depth
    let orig_depth = image.depth();
    // convert to floating point since hsl wants floating point
    image.convert_depth(BitDepth::Float32)?;

    // convert ignoring alpha
    for frame in image.frames_mut() {
        let channels = frame.channels_vec();
        let (r, rest) = channels.split_at_mut(1);
        let (g, b) = rest.split_at_mut(1);

        rgb_to_hsl(
            r[0].reinterpret_as_mut()?,
            g[0].reinterpret_as_mut()?,
            b[0].reinterpret_as_mut()?
        );
    }
    // restore original bit depth
    image.convert_depth(orig_depth)?;
    Ok(())
}

pub fn convert_hsl_to_rgb(image: &mut Image) -> Result<(), ImageErrors> {
    assert_eq!(image.colorspace(), ColorSpace::HSL);
    // preserve original depth
    let orig_depth = image.depth();
    // convert to floating point since hsl wants floating point
    image.convert_depth(BitDepth::Float32)?;

    // convert ignoring alpha
    for frame in image.frames_mut() {
        let channels = frame.channels_vec();
        let (r, rest) = channels.split_at_mut(1);
        let (g, b) = rest.split_at_mut(1);

        hsl_to_rgb(
            r[0].reinterpret_as_mut()?,
            g[0].reinterpret_as_mut()?,
            b[0].reinterpret_as_mut()?
        )
    }
    // restore original bit depth
    image.convert_depth(orig_depth)?;
    Ok(())
}
pub fn convert_rgb_to_hsv(image: &mut Image) -> Result<(), ImageErrors> {
    image.convert_color(ColorSpace::RGB)?; // recursive functions, what could go wrong

    // preserve original depth
    let orig_depth = image.depth();
    // convert to floating point since hsl wants floating point
    image.convert_depth(BitDepth::Float32)?;

    // convert ignoring alpha
    for frame in image.frames_mut() {
        let channels = frame.channels_vec();
        let (r, rest) = channels.split_at_mut(1);
        let (g, b) = rest.split_at_mut(1);

        rgb_to_hsv(
            r[0].reinterpret_as_mut()?,
            g[0].reinterpret_as_mut()?,
            b[0].reinterpret_as_mut()?
        );
    }
    // restore original bit depth
    image.convert_depth(orig_depth)?;

    Ok(())
}
pub fn convert_hsv_to_rgb(image: &mut Image) -> Result<(), ImageErrors> {
    assert_eq!(image.colorspace(), ColorSpace::HSV);
    // preserve original depth
    let orig_depth = image.depth();
    // convert to floating point since hsl wants floating point
    image.convert_depth(BitDepth::Float32)?;

    // convert ignoring alpha
    for frame in image.frames_mut() {
        let channels = frame.channels_vec();
        let (r, rest) = channels.split_at_mut(1);
        let (g, b) = rest.split_at_mut(1);

        hsv_to_rgb(
            r[0].reinterpret_as_mut()?,
            g[0].reinterpret_as_mut()?,
            b[0].reinterpret_as_mut()?
        )
    }
    // restore original bit depth
    image.convert_depth(orig_depth)?;

    Ok(())
}
pub fn pop_channel(image: &mut Image) {
    // contain the alpha channel
    for frame in image.frames_mut() {
        frame.channels_vec().pop().unwrap();
    }
}
pub fn convert_rgb_to_argb(image: &mut Image) -> Result<(), ImageErrors> {
    convert_adding_opaque_alpha(image)?;
    // swap
    for frame in image.frames_mut() {
        // switch the channels now
        let a_channel = frame.channels_vec().pop().unwrap();
        frame.channels_vec().insert(0, a_channel);
    }
    Ok(())
}

pub fn convert_rgba_to_argb_or_vice_versa(image: &mut Image) -> Result<(), ImageErrors> {
    assert!(matches!(
        image.colorspace(),
        ColorSpace::RGBA | ColorSpace::ARGB
    ));
    // swap
    for frame in image.frames_mut() {
        // switch the channels now
        let a_channel = frame.channels_vec().pop().unwrap();
        frame.channels_vec().insert(0, a_channel);
    }
    Ok(())
}

pub fn convert_luma_to_rgb(
    image: &mut Image, out_colorspace: ColorSpace
) -> Result<(), ImageErrors> {
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

            if !out_colorspace.has_alpha() {
                // if we don't expect alpha, remove it, otherwise continue
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
