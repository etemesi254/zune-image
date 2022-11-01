use zune_imageprocs::flip::flip;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct Flip;

impl Flip
{
    pub fn new() -> Flip
    {
        Self::default()
    }
}
impl OperationsTrait for Flip
{
    fn get_name(&self) -> &'static str
    {
        "Flip"
    }

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(input) =>
            {
                flip(input);
            }
            ImageChannels::TwoChannels(input) =>
            {
                for inp in input
                {
                    flip(inp);
                }
            }
            ImageChannels::ThreeChannels(input) =>
            {
                for inp in input
                {
                    flip(inp);
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                for inp in input
                {
                    flip(inp);
                }
            }
            ImageChannels::Interleaved(_) =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot flip interleaved pixels \
                de-interleave the pixels into separate color components first",
                ));
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot flip uninitialized pixels",
                ))
            }
        }
        Ok(())
    }
}
