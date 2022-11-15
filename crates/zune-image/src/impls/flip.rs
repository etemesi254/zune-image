use zune_imageprocs::flip::flip;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        for inp in image.get_channels_mut(true)
        {
            flip(inp);
        }

        Ok(())
    }
}
