use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::impls::grayscale::RgbToGrayScale;
use crate::traits::OperationsTrait;

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
                let converter = RgbToGrayScale::new().preserve_alpha(self.to.has_alpha());
                converter.execute(image).unwrap();
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
