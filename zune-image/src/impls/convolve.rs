use log::trace;
use zune_core::bit_depth::BitType;
use zune_imageprocs::convolve::convolve_1d;

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Convolve an image
#[derive(Default)]
pub struct Convolve
{
    weights: Vec<f64>
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();
        let max_val = image.get_depth().max_value();
        let depth = image.get_depth();

        #[cfg(feature = "threads")]
        {
            trace!("Running convolve in multithreaded mode");

            std::thread::scope(|s| {
                for channel in image.get_channels_mut(true)
                {
                    s.spawn(|| {
                        // Hello
                        let mut out_channel =
                            Channel::new_with_length(width * height * depth.size_of());

                        match depth.bit_type()
                        {
                            BitType::Eight =>
                            {
                                convolve_1d(
                                    channel.reinterpret_as::<u8>().unwrap(),
                                    out_channel.reinterpret_as_mut::<u8>().unwrap(),
                                    width,
                                    height,
                                    &self.weights,
                                    self.weights.len() as f64,
                                    max_val
                                );
                                *channel = out_channel;
                            }
                            BitType::Sixteen =>
                            {
                                convolve_1d(
                                    channel.reinterpret_as::<u16>().unwrap(),
                                    out_channel.reinterpret_as_mut::<u16>().unwrap(),
                                    width,
                                    height,
                                    &self.weights,
                                    self.weights.len() as f64,
                                    max_val
                                );
                                *channel = out_channel;
                            }
                            _ => todo!()
                        }
                    });
                }
            });
        }
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running convolve in single threaded mode");

            for channel in image.get_channels_mut(false)
            {
                let mut out_channel = Channel::new_with_length(width * height * depth.size_of());

                match depth.bit_type()
                {
                    BitType::Eight =>
                    {
                        convolve_1d(
                            channel.reinterpret_as::<u8>().unwrap(),
                            out_channel.reinterpret_as_mut::<u8>().unwrap(),
                            width,
                            height,
                            &self.weights,
                            self.weights.len() as f64,
                            max_val
                        );
                        *channel = out_channel;
                    }
                    BitType::Sixteen =>
                    {
                        convolve_1d(
                            channel.reinterpret_as::<u16>().unwrap(),
                            out_channel.reinterpret_as_mut::<u16>().unwrap(),
                            width,
                            height,
                            &self.weights,
                            self.weights.len() as f64,
                            max_val
                        );
                        *channel = out_channel;
                    }
                    _ => todo!()
                }
            }
        }
        Ok(())
    }
}
