use log::warn;
use zune_imageprocs::threshold::threshold;
pub use zune_imageprocs::threshold::ThresholdMethod;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

pub struct Threshold
{
    method:    ThresholdMethod,
    threshold: u16,
}

impl Threshold
{
    pub fn new(threshold: u16, method: ThresholdMethod) -> Threshold
    {
        Threshold { method, threshold }
    }
}
impl OperationsTrait for Threshold
{
    fn get_name(&self) -> &'static str
    {
        "Threshold"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        if !image.get_colorspace().is_grayscale()
        {
            warn!("Threshold works well with grayscale images, results may be something you don't expect")
        }

        for channel in image.get_channels_mut(false)
        {
            threshold(channel, self.threshold, self.method)
        }

        Ok(())
    }
}
