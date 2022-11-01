use zune_core::colorspace::ColorSpace;
use zune_imageprocs::invert::invert;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
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

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        match image.get_colorspace()
        {
            ColorSpace::YCbCr | ColorSpace::YCCK | ColorSpace::CYMK =>
            {
                return Err(ImgOperationsErrors::GenericString(format!(
                    "Invert operation is not implemented for {:?} colorspace",
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
                invert(input);
            }
            ImageChannels::TwoChannels(input) =>
            {
                invert(&mut input[0]);
            }
            ImageChannels::ThreeChannels(input) =>
            {
                for inp in input
                {
                    invert(inp);
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                // Assume 4th channel is alpha channel
                for inp in input.iter_mut().take(3)
                {
                    invert(inp);
                }
            }
            ImageChannels::Interleaved(data) =>
            {
                invert(data);
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot invert uninitialized pixels",
                ))
            }
        }
        Ok(())
    }
}
