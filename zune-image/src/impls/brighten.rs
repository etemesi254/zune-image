use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::brighten::brighten;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

#[derive(Default)]
pub struct Brighten
{
    value: i16
}

impl Brighten
{
    pub fn new(value: i16) -> Brighten
    {
        Brighten { value }
    }
}
impl OperationsTrait for Brighten
{
    fn get_name(&self) -> &'static str
    {
        "Brighten"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let max_val = image.get_depth().max_value();
        let depth = image.get_depth();

        for channel in image.get_channels_mut(true)
        {
            match depth.bit_type()
            {
                BitType::U8 => brighten(
                    channel.reinterpret_as_mut::<u8>().unwrap(),
                    self.value as u8,
                    max_val as u8
                ),
                BitType::Sixteen => brighten(
                    channel.reinterpret_as_mut::<u16>().unwrap(),
                    self.value as u16,
                    max_val
                ),
                _ => todo!()
            }
        }
        Ok(())
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
        &[BitType::U8, BitType::Sixteen]
    }
}
