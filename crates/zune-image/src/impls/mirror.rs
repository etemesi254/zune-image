use zune_imageprocs::mirror::mirror;
pub use zune_imageprocs::mirror::MirrorMode;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;
/// Rearrange the pixels up side down
pub struct Mirror
{
    mode: MirrorMode,
}

impl Mirror
{
    pub fn new(mode: MirrorMode) -> Mirror
    {
        Self { mode }
    }
}
impl OperationsTrait for Mirror
{
    fn get_name(&self) -> &'static str
    {
        "Mirror"
    }

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();
        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(input) =>
            {
                mirror(input, width, height, self.mode);
            }
            ImageChannels::TwoChannels(input) =>
            {
                for inp in input
                {
                    mirror(inp, width, height, self.mode);
                }
            }
            ImageChannels::ThreeChannels(input) =>
            {
                for inp in input
                {
                    mirror(inp, width, height, self.mode);
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                for inp in input
                {
                    mirror(inp, width, height, self.mode);
                }
            }
            ImageChannels::Interleaved(_) =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot mirror interleaved pixels \
                de-interleave the pixels into separate color components first",
                ));
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot mirror uninitialized pixels",
                ))
            }
        }
        Ok(())
    }
}
