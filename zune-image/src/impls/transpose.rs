use zune_core::colorspace::ColorSpace;
use zune_imageprocs::transpose::transpose;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

pub struct Transpose;

impl Transpose
{
    pub fn new() -> Transpose
    {
        return Transpose {};
    }
}
impl OperationsTrait for Transpose
{
    fn get_name(&self) -> &'static str
    {
        "Transpose"
    }

    fn execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let colorspace = image.get_colorspace();
        let (im_width, im_height) = image.get_dimensions();

        let channels = image.get_channel_mut();

        match colorspace
        {
            ColorSpace::RGB | ColorSpace::YCbCr => match channels
            {
                ImageChannels::ThreeChannels(data) =>
                {
                    let dimensions = im_width * im_height;

                    let mut c1 = vec![0_u8; dimensions];
                    transpose(&data[0], &mut c1, im_width, im_height);

                    let mut c2 = vec![0_u8; dimensions];
                    transpose(&data[1], &mut c2, im_width, im_height);

                    let mut c3 = vec![0_u8; dimensions];
                    transpose(&data[2], &mut c3, im_width, im_height);

                    let new_channel = ImageChannels::ThreeChannels([c1, c2, c3]);
                    *channels = new_channel;

                    //set width and height to be opposite
                    image.set_dimensions(im_height, im_width);
                }

                _ => unimplemented!(),
            },
            ColorSpace::GrayScale => match channels
            {
                ImageChannels::OneChannel(data) =>
                {
                    let dimensions = im_width * im_height;

                    let mut c1 = vec![0_u8; dimensions];
                    transpose(data, &mut c1, im_width, im_height);

                    let new_channel = ImageChannels::OneChannel(c1);
                    *channels = new_channel;

                    image.set_dimensions(im_height, im_width);
                }
                _ => unimplemented!(),
            },
            _ => panic!(),
        }
        Ok(())
    }
}
