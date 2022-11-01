use log::warn;
use zune_imageprocs::threshold::threshold;
pub use zune_imageprocs::threshold::ThresholdMethod;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

pub struct Threshold
{
    method:    ThresholdMethod,
    threshold: u8,
}

impl Threshold
{
    pub fn new(threshold: u8, method: ThresholdMethod) -> Threshold
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

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(input) =>
            {
                threshold(input, self.threshold, self.method);
            }
            ImageChannels::TwoChannels(input) =>
            {
                threshold(&mut input[0], self.threshold, self.method);
            }
            ImageChannels::ThreeChannels(input) =>
            {
                warn!("Threshold expects A grayscale image, results may not be what you expect");

                for inp in input
                {
                    threshold(inp, self.threshold, self.method);
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                warn!("Threshold expects A grayscale image, results may not be what you expect");
                for inp in input.iter_mut().take(3)
                {
                    threshold(inp, self.threshold, self.method);
                }
            }
            ImageChannels::Interleaved(data) =>
            {
                threshold(data, self.threshold, self.method);
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot threshold uninitialized pixels",
                ))
            }
        }
        Ok(())
    }
}
