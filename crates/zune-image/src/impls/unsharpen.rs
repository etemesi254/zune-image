use log::trace;
use zune_imageprocs::unsharpen::unsharpen;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Perform an unsharpen mask
#[derive(Default)]
pub struct Unsharpen
{
    sigma:     f32,
    threshold: u16,
}

impl Unsharpen
{
    pub fn new(sigma: f32, threshold: u16) -> Unsharpen
    {
        Unsharpen { sigma, threshold }
    }
}

impl OperationsTrait for Unsharpen
{
    fn get_name(&self) -> &'static str
    {
        "Unsharpen"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        #[cfg(not(feature = "threads"))]
        {
            let mut blur_buffer = vec![0; width * height];
            let mut blur_scratch = vec![0; width * height];

            for channel in image.get_channels_mut(false)
            {
                unsharpen(
                    channel,
                    &mut blur_buffer,
                    &mut blur_scratch,
                    self.sigma,
                    self.threshold,
                    width,
                    height,
                );
            }
        }

        #[cfg(feature = "threads")]
        {
            trace!("Running unsharpen in multithreaded mode");
            std::thread::scope(|s| {
                // blur each channel on a separate thread
                for channel in image.get_channels_mut(false)
                {
                    s.spawn(|| {
                        let mut blur_buffer = vec![0; width * height];
                        let mut blur_scratch = vec![0; width * height];

                        unsharpen(
                            channel,
                            &mut blur_buffer,
                            &mut blur_scratch,
                            self.sigma,
                            self.threshold,
                            width,
                            height,
                        );
                    });
                }
            });
        }

        Ok(())
    }
}
