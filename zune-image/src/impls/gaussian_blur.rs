use log::trace;
use zune_imageprocs::gaussian_blur::gaussian_blur;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
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

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(channel) =>
            {
                let mut out_dim = vec![0; width * height];
                gaussian_blur(channel, &mut out_dim, width, height, self.sigma);
            }
            ImageChannels::TwoChannels(channels) =>
            {
                let mut out_dim = vec![0; width * height];
                gaussian_blur(&mut channels[0], &mut out_dim, width, height, self.sigma);
                //channels[0] = out_dim;
            }
            ImageChannels::ThreeChannels(channels) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running gaussian blur in single threaded mode");
                    let mut out_dim = vec![0; width * height];

                    for channel in channels
                    {
                        gaussian_blur(channel, &mut out_dim, width, height, self.sigma);

                        // *channel = out_dim;
                    }
                }
                #[cfg(feature = "threads")]
                {
                    trace!("Running gaussian blur in multithreaded mode");
                    std::thread::scope(|s| {
                        // blur each channel on a separate thread
                        for channel in channels
                        {
                            s.spawn(|| {
                                let mut out_dim = vec![0; width * height];
                                gaussian_blur(channel, &mut out_dim, width, height, self.sigma);
                            });
                        }
                    });
                }
            }
            ImageChannels::FourChannels(channels) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running gaussian blur in single threaded mode");
                    let mut out_dim = vec![0; width * height];

                    for channel in channels.iter_mut().take(3)
                    {
                        gaussian_blur(channel, &mut out_dim, width, height, self.radius);
                    }
                }
                #[cfg(feature = "threads")]
                {
                    trace!("Running gaussian blur in multithreaded mode");
                    std::thread::scope(|s| {
                        // blur each channel on a separate thread
                        for channel in channels.iter_mut().take(3)
                        {
                            s.spawn(|| {
                                let mut out_dim = vec![0; width * height];

                                gaussian_blur(channel, &mut out_dim, width, height, self.sigma);
                            });
                        }
                    });
                }
            }
            ImageChannels::Interleaved(_) =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot blur an interleaved channel",
                ));
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot blur uninitialized pixels",
                ));
            }
        }
        Ok(())
    }
}
