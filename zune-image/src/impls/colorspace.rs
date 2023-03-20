use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
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

fn convert_rgb_to_rgba(image: &mut Image) -> Result<(), ImgOperationsErrors>
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
        _ =>
        {
            return Err(ImgOperationsErrors::Generic(
                "Unsupported bit depth for RGB->RGBA conversion"
            ))
        }
    };
    image.channels.push(new_channel);

    Ok(())
}

impl OperationsTrait for ColorspaceConv
{
    fn get_name(&self) -> &'static str
    {
        "Colorspace conversion"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let from = image.get_colorspace();

        match (from, self.to)
        {
            (ColorSpace::RGB, ColorSpace::RGBA) =>
            {
                convert_rgb_to_rgba(image)?;
            }

            (ColorSpace::RGB | ColorSpace::RGBA, ColorSpace::Luma | ColorSpace::LumaA) =>
            {
                // use the rgb to grayscale converter
                let converter = RgbToGrayScale::new().preserve_alpha(self.to.has_alpha());
                converter.execute(image).unwrap();
            }

            (a, b) =>
            {
                let msg = format!("Unsupported/unknown mapping from {a:?} to {b:?}");
                return Err(ImgOperationsErrors::GenericString(msg));
            }
        }
        // set it to the new colorspace
        image.set_colorspace(self.to);

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U16, BitType::U8]
    }
}
