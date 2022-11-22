use zune_imageprocs::crop::crop;

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

pub struct Crop
{
    x:      usize,
    y:      usize,
    width:  usize,
    height: usize
}

impl Crop
{
    pub fn new(width: usize, height: usize, x: usize, y: usize) -> Crop
    {
        Crop {
            x,
            y,
            width,
            height
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
        let new_dims = self.width * self.height * image.get_depth().size_of();
        let (old_width, _) = image.get_dimensions();

        for channel in image.get_channels_mut(true)
        {
            let mut new_vec = Channel::new_with_length(new_dims);

            // since crop is just bytewise copies, we can use the lowest common denominator for it
            // and it will still work
            crop(
                channel.reinterpret_as::<u8>().unwrap(),
                old_width,
                new_vec.reinterpret_as_mut::<u8>().unwrap(),
                self.width,
                self.height,
                self.x,
                self.y
            );
            *channel = new_vec;
        }

        image.set_dimensions(self.width, self.height);

        Ok(())
    }
}
