use log::trace;
use zune_imageprocs::gamma::gamma;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
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

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(input) =>
            {
                gamma(input, self.value);
            }
            ImageChannels::TwoChannels(input) =>
            {
                for inp in input
                {
                    gamma(inp, self.value);
                }
            }
            ImageChannels::ThreeChannels(input) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running gamma correction in single threaded mode");

                    for inp in input
                    {
                        gamma(inp, self.value);
                    }
                }
                #[cfg(feature = "threads")]
                {
                    trace!("Running gamma correction in multithreaded mode");
                    std::thread::scope(|s| {
                        // blur each channel on a separate thread
                        for channel in input
                        {
                            s.spawn(|| {
                                gamma(channel, self.value);
                            });
                        }
                    });
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running gamma correction in single threaded mode");

                    for inp in input.iter_mut().take(3)
                    {
                        gamma(inp, self.value);
                    }
                }
                #[cfg(feature = "threads")]
                {
                    trace!("Running gamma correction in multithreaded mode");

                    std::thread::scope(|s| {
                        // blur each channel on a separate thread
                        for channel in input.iter_mut().take(3)
                        {
                            s.spawn(|| {
                                gamma(channel, self.value);
                            });
                        }
                    });
                }
            }
            ImageChannels::Interleaved(input) =>
            {
                gamma(input, self.value);
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::InvalidChannelLayout(
                    "Cannot gamma adjust uninitialized pixels",
                ))
            }
        }
        Ok(())
    }
}
