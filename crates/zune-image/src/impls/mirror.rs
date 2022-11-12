use zune_imageprocs::mirror::mirror;
pub use zune_imageprocs::mirror::MirrorMode;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
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

        for channel in image.get_channels_mut(true)
        {
            mirror(channel, width, height, self.mode);
        }

        Ok(())
    }
}
