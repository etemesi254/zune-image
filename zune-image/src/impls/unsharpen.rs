use log::trace;
use zune_imageprocs::unsharpen::unsharpen;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

/// Perform an unsharpen mask
#[derive(Default)]
pub struct Unsharpen
{
    sigma:     f32,
    threshold: u8,
}

impl Unsharpen
{
    pub fn new(sigma: f32, threshold: u8) -> Unsharpen
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

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(channel) =>
            {
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
            }
            ImageChannels::TwoChannels(channels) =>
            {
                let mut blur_buffer = vec![0; width * height];
                let mut blur_scratch = vec![0; width * height];

                unsharpen(
                    &mut channels[0],
                    &mut blur_buffer,
                    &mut blur_scratch,
                    self.sigma,
                    self.threshold,
                    width,
                    height,
                );
            }
            ImageChannels::ThreeChannels(channels) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running unsharpen in single threaded mode");
                    let mut blur_buffer = vec![0; width * height];
                    let mut blur_scratch = vec![0; width * height];

                    for channel in channels
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
                        for channel in channels
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
            }
            ImageChannels::FourChannels(channels) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running unsharpen in single threaded mode");
                    let mut blur_buffer = vec![0; width * height];
                    let mut blur_scratch = vec![0; width * height];

                    for channel in channels.iter_mut().take(3)
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
                        for channel in channels.iter_mut().take(3)
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
