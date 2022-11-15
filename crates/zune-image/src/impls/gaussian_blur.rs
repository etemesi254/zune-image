use log::trace;
use zune_imageprocs::gaussian_blur::gaussian_blur;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Perform a gaussian blur
#[derive(Default)]
pub struct GaussianBlur
{
    sigma: f32,
}

impl GaussianBlur
{
    pub fn new(sigma: f32) -> GaussianBlur
    {
        GaussianBlur { sigma }
    }
}

impl OperationsTrait for GaussianBlur
{
    fn get_name(&self) -> &'static str
    {
        "Gaussian blur"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        #[cfg(not(feature = "threads"))]
        {
            trace!("Running gaussian blur in single threaded mode");

            let mut temp = vec![0; width * height];

            for channel in image.get_channels_mut(false)
            {
                gaussian_blur(channel, &mut temp, width, height, self.sigma);
            }
        }

        #[cfg(feature = "threads")]
        {
            trace!("Running gaussian blur in multithreaded mode");
            std::thread::scope(|s| {
                // blur each channel on a separate thread
                for channel in image.get_channels_mut(false)
                {
                    s.spawn(|| {
                        let mut temp = vec![0; width * height];

                        gaussian_blur(channel, &mut temp, width, height, self.sigma);
                    });
                }
            });
        }

        Ok(())
    }
}
