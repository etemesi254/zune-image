use log::trace;
use zune_imageprocs::box_blur::box_blur;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

/// Perform a box blur with a normal gaussian
/// kernel with the following weights
///
/// ```text
/// 1 [[1,1,1]
/// _  [1,1,1]
/// 9  [1,1,1]]
/// ```
#[derive(Default)]
pub struct BoxBlur
{
    radius: usize,
}

impl BoxBlur
{
    pub fn new(radius: usize) -> BoxBlur
    {
        BoxBlur { radius }
    }
}

impl OperationsTrait for BoxBlur
{
    fn get_name(&self) -> &'static str
    {
        "Box blur"
    }

    fn execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(channel) =>
            {
                let mut out_dim = vec![0; width * height];
                box_blur(channel, &mut out_dim, width, height, self.radius);
                //*channel = out_dim;
            }
            ImageChannels::TwoChannels(channels) =>
            {
                let mut out_dim = vec![0; width * height];
                box_blur(&mut channels[0], &mut out_dim, width, height, self.radius);
                //channels[0] = out_dim;
            }
            ImageChannels::ThreeChannels(channels) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running box blur in single threaded mode");
                    let mut out_dim = vec![0; width * height];

                    for channel in channels
                    {
                        box_blur(channel, &mut out_dim, width, height, self.radius);

                        // *channel = out_dim;
                    }
                }
                #[cfg(feature = "threads")]
                {
                    trace!("Running box blur in multithreaded mode");
                    std::thread::scope(|s| {
                        // blur each channel on a separate thread
                        for channel in channels
                        {
                            s.spawn(|| {
                                let mut out_dim = vec![0; width * height];
                                box_blur(channel, &mut out_dim, width, height, self.radius);
                            });
                        }
                    });
                }
            }
            ImageChannels::FourChannels(channels) =>
            {
                #[cfg(not(feature = "threads"))]
                {
                    trace!("Running box blur in single threaded mode");
                    let mut out_dim = vec![0; width * height];

                    for channel in channels.iter_mut().take(3)
                    {
                        box_blur(channel, &mut out_dim, width, height, self.radius);

                        // *channel = out_dim;
                    }
                }
                #[cfg(feature = "threads")]
                {
                    trace!("Running box blur in multithreaded mode");
                    std::thread::scope(|s| {
                        // blur each channel on a separate thread
                        for channel in channels.iter_mut().take(3)
                        {
                            s.spawn(|| {
                                let mut out_dim = vec![0; width * height];
                                box_blur(channel, &mut out_dim, width, height, self.radius);
                            });
                        }
                        // let mut out_dim = vec![0; width * height];
                        //
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
