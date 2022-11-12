use log::trace;
use zune_imageprocs::convolve::convolve_1d;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct Convolve
{
    weights: Vec<f64>,
}

impl Convolve
{
    pub fn new(weights: Vec<f64>) -> Convolve
    {
        Convolve { weights }
    }
}

impl OperationsTrait for Convolve
{
    fn get_name(&self) -> &'static str
    {
        "1D convolution"
    }

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();
        let max_val = image.get_depth().max_value();

        #[cfg(feature = "threads")]
        {
            trace!("Running convolve in multithreaded mode");

            std::thread::scope(|s| {
                for channel in image.get_channels_mut(false)
                {
                    s.spawn(|| {
                        let mut out_channel = vec![0; channel.len()];
                        convolve_1d(
                            channel,
                            &mut out_channel,
                            width,
                            height,
                            &self.weights,
                            self.weights.len() as f64,
                            max_val,
                        );

                        *channel = out_channel;
                    });
                }
            });
        }
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running convolve in single threaded mode");

            for channel in image.get_channels_mut(false)
            {
                let mut out_channel = vec![0; channel.len()];
                convolve_1d(
                    channel,
                    &mut out_channel,
                    width,
                    height,
                    &self.weights,
                    self.weights.len() as f64,
                    max_val,
                );
                *channel = out_channel;
            }
        }
        Ok(())
    }
}
