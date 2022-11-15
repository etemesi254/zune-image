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
        for channel in image.get_channels_mut(false)
        {
            brighten(channel, self.value, max_val)
        }
        Ok(())
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGBA,
            ColorSpace::RGBX,
            ColorSpace::RGB,
            ColorSpace::LumaA,
            ColorSpace::Luma
        ]
    }
}
