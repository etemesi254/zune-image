use log::warn;
use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::grayscale::{rgb_to_grayscale_u16, rgb_to_grayscale_u8};

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Convert RGB data to grayscale
///
/// This will convert any image that contains three
/// RGB channels(including RGB, RGBA,RGBX) into grayscale
///
/// Formula for RGB to grayscale conversion is given by
///
/// ```text
///Grayscale = 0.299R + 0.587G + 0.114B
/// ```
/// but it's implemented using fixed point integer mathematics and simd kernels
/// where applicable (see zune-imageprocs/grayscale)
pub struct RgbToGrayScale
{
    preserve_alpha: bool
}

impl RgbToGrayScale
{
    #[allow(clippy::new_without_default)]
    pub fn new() -> RgbToGrayScale
    {
        RgbToGrayScale {
            preserve_alpha: false
        }
    }
    pub fn preserve_alpha(mut self, yes: bool) -> RgbToGrayScale
    {
        self.preserve_alpha = yes;
        self
    }
}
impl OperationsTrait for RgbToGrayScale
{
    fn get_name(&self) -> &'static str
    {
        "RGB to Grayscale"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let im_colorspace = image.get_colorspace();

        if im_colorspace == ColorSpace::Luma || im_colorspace == ColorSpace::LumaA
        {
            warn!("Image already in grayscale skipping this operation");
            return Ok(());
        }

        let (width, height) = image.get_dimensions();
        let size = width * height * image.get_depth().size_of();

        let channel = image.get_channels_ref(self.preserve_alpha);

        match image.get_depth().bit_type()
        {
            BitType::U8 =>
            {
                let r = channel[0].reinterpret_as::<u8>().unwrap();
                let g = channel[1].reinterpret_as::<u8>().unwrap();
                let b = channel[2].reinterpret_as::<u8>().unwrap();
                let mut out = Channel::new_with_length(size);

                rgb_to_grayscale_u8(
                    r,
                    g,
                    b,
                    out.reinterpret_as_mut::<u8>().unwrap(),
                    image.get_depth().max_value() as u8
                );

                if self.preserve_alpha && image.get_colorspace().has_alpha()
                {
                    image.set_channels(vec![out, channel[3].clone()]);
                    image.set_colorspace(ColorSpace::LumaA);
                }
                else
                {
                    image.set_channels(vec![out]);
                    image.set_colorspace(ColorSpace::Luma);
                }

                Ok(())
            }
            BitType::U16 =>
            {
                let r = channel[0].reinterpret_as::<u16>().unwrap();
                let g = channel[1].reinterpret_as::<u16>().unwrap();
                let b = channel[2].reinterpret_as::<u16>().unwrap();
                let mut out = Channel::new_with_length(size);

                rgb_to_grayscale_u16(
                    r,
                    g,
                    b,
                    out.reinterpret_as_mut::<u16>().unwrap(),
                    image.get_depth().max_value()
                );

                if self.preserve_alpha && image.get_colorspace().has_alpha()
                {
                    image.set_channels(vec![out, channel[3].clone()]);
                    image.set_colorspace(ColorSpace::LumaA);
                }
                else
                {
                    image.set_channels(vec![out]);
                    image.set_colorspace(ColorSpace::Luma);
                }

                Ok(())
            }
            _ => todo!()
        }
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGBA,
            ColorSpace::RGB,
            ColorSpace::LumaA,
            ColorSpace::Luma
        ]
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8, BitType::U16]
    }
}
