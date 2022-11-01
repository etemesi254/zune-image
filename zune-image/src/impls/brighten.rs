use zune_core::colorspace::ColorSpace;
use zune_imageprocs::brighten::brighten;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

/// Invert
#[derive(Default)]
pub struct Brighten
{
    value: i16,
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

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        match image.get_colorspace()
        {
            ColorSpace::YCbCr | ColorSpace::YCCK | ColorSpace::CYMK =>
            {
                return Err(ImgOperationsErrors::GenericString(format!(
                    "Brighten operation is not implemented for {:?} colorspace ",
                    image.get_colorspace()
                )))
            }
            _ =>
            {}
        };

        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(input) =>
            {
                brighten(input, self.value);
            }
            ImageChannels::TwoChannels(input) =>
            {
                for inp in input
                {
                    brighten(inp, self.value);
                }
            }
            ImageChannels::ThreeChannels(input) =>
            {
                for inp in input
                {
                    brighten(inp, self.value);
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                for inp in input.iter_mut().take(3)
                {
                    brighten(inp, self.value);
                }
            }
            ImageChannels::Interleaved(data) =>
            {
                brighten(data, self.value);
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot brighten uninitialized pixels",
                ))
            }
        }
        Ok(())
    }
}
