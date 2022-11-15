use log::trace;
use zune_imageprocs::gamma::gamma;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct Gamma
{
    value: f32,
}

impl Gamma
{
    pub fn new(value: f32) -> Gamma
    {
        Gamma { value }
    }
}
impl OperationsTrait for Gamma
{
    fn get_name(&self) -> &'static str
    {
        "Gamma Correction"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let max_value = image.get_depth().max_value();

        #[cfg(not(feature = "threads"))]
        {
            trace!("Running gamma correction in single threaded mode");

            for channel in image.get_channels_mut(false)
            {
                gamma(channel, self.value, max_value);
            }
        }
        #[cfg(feature = "threads")]
        {
            trace!("Running gamma correction in multithreaded mode");

            std::thread::scope(|s| {
                for channel in image.get_channels_mut(false)
                {
                    s.spawn(|| gamma(channel, self.value, max_value));
                }
            });
        }

        Ok(())
    }
}
