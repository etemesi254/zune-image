use zune_imageprocs::flop::flop;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct Flop;

impl Flop
{
    pub fn new() -> Flop
    {
        Self::default()
    }
}
impl OperationsTrait for Flop
{
    fn get_name(&self) -> &'static str
    {
        "Flop"
    }

    fn execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, _) = image.get_dimensions();

        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(input) =>
            {
                flop(input, width);
            }
            ImageChannels::TwoChannels(input) =>
            {
                for inp in input
                {
                    flop(inp, width);
                }
            }
            ImageChannels::ThreeChannels(input) =>
            {
                for inp in input
                {
                    flop(inp, width);
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                for inp in input
                {
                    flop(inp, width);
                }
            }
            ImageChannels::Interleaved(_) =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot flop interleaved pixels \
                de-interleave the pixels into separate color components first",
                ));
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot flop uninitialized pixels",
                ))
            }
        }
        Ok(())
    }
}
