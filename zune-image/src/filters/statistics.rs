use log::trace;
use zune_core::bit_depth::BitType;
use zune_imageprocs::spatial_ops::spatial_ops;
pub use zune_imageprocs::spatial_ops::StatisticOperations;

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Median returns a new image in which each pixel is the median of its neighbors.
///
/// The parameter radius corresponds to the radius of the neighbor area to be searched,
///
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
pub struct StatisticsOps
{
    radius:    usize,
    operation: StatisticOperations
}

impl StatisticsOps
{
    pub fn new(radius: usize, operation: StatisticOperations) -> StatisticsOps
    {
        StatisticsOps { radius, operation }
    }
}

impl OperationsTrait for StatisticsOps
{
    fn get_name(&self) -> &'static str
    {
        "StatisticsOps Filter"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let (width, height) = image.get_dimensions();

        let depth = image.get_depth();
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running erode filter in single threaded mode");

            for channel in image.get_channels_mut(true)
            {
                let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

                match depth.bit_type()
                {
                    BitType::U16 => spatial_ops(
                        channel.reinterpret_as::<u16>().unwrap(),
                        new_channel.reinterpret_as_mut::<u16>().unwrap(),
                        self.radius,
                        width,
                        height,
                        self.operation
                    ),
                    BitType::U8 => spatial_ops(
                        channel.reinterpret_as::<u8>().unwrap(),
                        new_channel.reinterpret_as_mut::<u8>().unwrap(),
                        self.radius,
                        width,
                        height,
                        self.operation
                    ),
                    _ => todo!()
                }
                *channel = new_channel;
            }
        }
        #[cfg(feature = "threads")]
        {
            trace!(
                "Running statistics filter  for {:?} in multithreaded mode",
                self.operation
            );

            std::thread::scope(|s| {
                for channel in image.get_channels_mut(false)
                {
                    s.spawn(|| {
                        let mut new_channel =
                            Channel::new_with_bit_type(channel.len(), depth.bit_type());

                        match depth.bit_type()
                        {
                            BitType::U16 => spatial_ops(
                                channel.reinterpret_as::<u16>().unwrap(),
                                new_channel.reinterpret_as_mut::<u16>().unwrap(),
                                self.radius,
                                width,
                                height,
                                self.operation
                            ),
                            BitType::U8 => spatial_ops(
                                channel.reinterpret_as::<u8>().unwrap(),
                                new_channel.reinterpret_as_mut::<u8>().unwrap(),
                                self.radius,
                                width,
                                height,
                                self.operation
                            ),
                            _ => todo!()
                        }
                        *channel = new_channel;
                    });
                }
            });
        }
        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8, BitType::U16]
    }
}
