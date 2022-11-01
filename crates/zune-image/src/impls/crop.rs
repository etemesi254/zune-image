use zune_imageprocs::crop::crop;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
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

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let new_dims = self.width * self.height;
        let (old_width, _) = image.get_dimensions();
        let colorspace = image.get_colorspace();

        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(input) =>
            {
                let mut new_vec = vec![0; new_dims];

                crop(
                    input,
                    old_width,
                    &mut new_vec,
                    self.width,
                    self.height,
                    self.x,
                    self.y,
                );
                *input = new_vec;
            }
            ImageChannels::TwoChannels(input) =>
            {
                for inp in input
                {
                    let mut new_vec = vec![0; new_dims];

                    crop(
                        inp,
                        old_width,
                        &mut new_vec,
                        self.width,
                        self.height,
                        self.x,
                        self.y,
                    );
                    *inp = new_vec;
                }
            }
            ImageChannels::ThreeChannels(input) =>
            {
                for inp in input
                {
                    let mut new_vec = vec![0; new_dims];

                    crop(
                        inp,
                        old_width,
                        &mut new_vec,
                        self.width,
                        self.height,
                        self.x,
                        self.y,
                    );
                    *inp = new_vec;
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                for inp in input.iter_mut().take(3)
                {
                    let mut new_vec = vec![0; new_dims];

                    crop(
                        inp,
                        old_width,
                        &mut new_vec,
                        self.width,
                        self.height,
                        self.x,
                        self.y,
                    );
                    *inp = new_vec;
                }
            }
            ImageChannels::Interleaved(data) =>
            {
                // Cropping an interleaved image is a bit different
                // Now a width is an image width plus number of pixels per component.
                let mut new_vec = vec![0; new_dims * colorspace.num_components()];

                crop(
                    data,
                    old_width * colorspace.num_components(),
                    &mut new_vec,
                    self.width * colorspace.num_components(),
                    self.height,
                    self.x,
                    self.y,
                );
                *data = new_vec;
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot crop uninitialized pixels",
                ))
            }
        }
        image.set_dimensions(self.width, self.height);

        Ok(())
    }
}
