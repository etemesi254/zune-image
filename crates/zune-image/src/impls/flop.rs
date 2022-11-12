use zune_imageprocs::flop::flop;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
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

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, _) = image.get_dimensions();
        for channel in image.get_channels_mut(true)
        {
            flop(channel, width);
        }

        Ok(())
    }
}
