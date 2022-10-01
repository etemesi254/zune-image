use zune_core::colorspace::ColorSpace;
use zune_imageprocs::grayscale::rgb_to_grayscale;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

pub struct RgbToGrayScale;

impl RgbToGrayScale
{
    #[allow(clippy::new_without_default)]
    pub fn new() -> RgbToGrayScale
    {
        RgbToGrayScale {}
    }
}
impl OperationsTrait for RgbToGrayScale
{
    fn get_name(&self) -> &'static str
    {
        "RGB to Grayscale"
    }

    fn execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        if image.get_colorspace() != ColorSpace::RGB
        {
            return Err(ImgOperationsErrors::WrongColorspace(
                ColorSpace::RGB,
                image.get_colorspace(),
            ));
        }

        let (width, height) = image.get_dimensions();
        let size = width * height;
        let mut grayscale = vec![0; size];

        if let ImageChannels::ThreeChannels(rgb_data) = image.get_channel_mut()
        {
            rgb_to_grayscale((&rgb_data[0], &rgb_data[1], &rgb_data[2]), &mut grayscale);
        }
        // change image info to be grayscale
        image.set_image_channel(ImageChannels::OneChannel(grayscale));
        image.set_colorspace(ColorSpace::GrayScale);

        Ok(())
    }
}
