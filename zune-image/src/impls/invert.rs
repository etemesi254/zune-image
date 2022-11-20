use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::invert::invert;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Invert
#[derive(Default)]
pub struct Invert;

impl Invert
{
    pub fn new() -> Invert
    {
        Self::default()
    }
}
impl OperationsTrait for Invert
{
    fn get_name(&self) -> &'static str
    {
        "Invert"
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let depth = image.get_depth().bit_type();

        for channel in image.get_channels_mut(false)
        {
            match depth
            {
                BitType::Eight => invert(channel.reinterpret_as_mut::<u8>().unwrap()),
                BitType::Sixteen => invert(channel.reinterpret_as_mut::<u16>().unwrap())
            }
        }

        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGB,
            ColorSpace::RGBA,
            ColorSpace::LumaA,
            ColorSpace::RGBX,
            ColorSpace::Luma
        ]
    }
}
