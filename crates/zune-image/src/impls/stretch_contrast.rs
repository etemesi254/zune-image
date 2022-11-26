use zune_core::bit_depth::BitType;
use zune_imageprocs::stretch_contrast::stretch_contrast;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Linearly stretches the contrast in an image in place,
/// sending lower to image minimum and upper to image maximum.
#[derive(Default)]
pub struct StretchContrast
{
    lower: u16,
    upper: u16
}

impl StretchContrast
{
    pub fn new(lower: u16, upper: u16) -> StretchContrast
    {
        StretchContrast { lower, upper }
    }
}

impl OperationsTrait for StretchContrast
{
    fn get_name(&self) -> &'static str
    {
        "Stretch Contrast"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let depth = image.get_depth();

        for channel in image.get_channels_mut(false)
        {
            match depth.bit_type()
            {
                BitType::Eight => stretch_contrast(
                    channel.reinterpret_as_mut::<u8>().unwrap(),
                    self.lower as u8,
                    self.upper as u8
                ),
                BitType::Sixteen => stretch_contrast(
                    channel.reinterpret_as_mut::<u16>().unwrap(),
                    self.lower,
                    self.upper
                )
            }
        }
        Ok(())
    }
}
