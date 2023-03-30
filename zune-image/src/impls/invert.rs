use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::invert::invert;

use crate::errors::ImageErrors;
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
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let depth = image.get_depth().bit_type();

        for channel in image.get_channels_mut(true)
        {
            match depth
            {
                BitType::U8 => invert(channel.reinterpret_as_mut::<u8>().unwrap()),
                BitType::U16 => invert(channel.reinterpret_as_mut::<u16>().unwrap()),
                _ => todo!()
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
            ColorSpace::Luma
        ]
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8, BitType::U16]
    }
}
