use log::trace;
use zune_core::bit_depth::BitType;
use zune_imageprocs::erode::erode;

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Median returns a new image in which each pixel is the median of its neighbors.
///
/// The parameter radius corresponds to the radius of the neighbor area to be searched,
///
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
#[derive(Default)]
pub struct Erode
{
    radius: usize
}

impl Erode
{
    pub fn new(radius: usize) -> Erode
    {
        Erode { radius }
    }
}

impl OperationsTrait for Erode
{
    fn get_name(&self) -> &'static str
    {
        "Erode Filter"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, _) = image.get_dimensions();

        let depth = image.get_depth();
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running erode filter in single threaded mode");

            for channel in image.get_channels_mut(false)
            {
                let mut new_channel = Channel::new_with_length(channel.len());

                match depth.bit_type()
                {
                    BitType::Sixteen => erode(
                        channel.reinterpret_as::<u16>().unwrap(),
                        new_channel.reinterpret_as_mut::<u16>().unwrap(),
                        self.radius,
                        width
                    ),
                    BitType::Eight => erode(
                        channel.reinterpret_as::<u8>().unwrap(),
                        new_channel.reinterpret_as_mut::<u8>().unwrap(),
                        self.radius,
                        width
                    )
                }
                *channel = new_channel;
            }
        }
        #[cfg(feature = "threads")]
        {
            trace!("Running erode filter in multithreaded mode");

            std::thread::scope(|s| {
                for channel in image.get_channels_mut(false)
                {
                    s.spawn(|| {
                        let mut new_channel = Channel::new_with_length(channel.len());

                        match depth.bit_type()
                        {
                            BitType::Sixteen => erode(
                                channel.reinterpret_as::<u16>().unwrap(),
                                new_channel.reinterpret_as_mut::<u16>().unwrap(),
                                self.radius,
                                width
                            ),
                            BitType::Eight => erode(
                                channel.reinterpret_as::<u8>().unwrap(),
                                new_channel.reinterpret_as_mut::<u8>().unwrap(),
                                self.radius,
                                width
                            )
                        }
                        *channel = new_channel;
                    });
                }
            });
        }
        Ok(())
    }
}
