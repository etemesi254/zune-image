use zune_imageprocs::crop::crop;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

pub struct Crop
{
    x:      usize,
    y:      usize,
    width:  usize,
    height: usize,
}

impl Crop
{
    pub fn new(width: usize, height: usize, x: usize, y: usize) -> Crop
    {
        Crop {
            x,
            y,
            width,
            height,
        }
    }
}

impl OperationsTrait for Crop
{
    fn get_name(&self) -> &'static str
    {
        "Crop"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let new_dims = self.width * self.height;
        let (old_width, _) = image.get_dimensions();

        for channel in image.get_channels_mut(true)
        {
            let mut new_vec = vec![0; new_dims];

            crop(
                channel,
                old_width,
                &mut new_vec,
                self.width,
                self.height,
                self.x,
                self.y,
            );
            *channel = new_vec;
        }

        image.set_dimensions(self.width, self.height);

        Ok(())
    }
}
