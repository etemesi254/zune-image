use log::trace;
use zune_core::bit_depth::BitType;
use zune_imageprocs::gaussian_blur::{gaussian_blur_u16, gaussian_blur_u8};

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Perform a gaussian blur
#[derive(Default)]
pub struct GaussianBlur
{
    sigma: f32
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let (width, height) = image.get_dimensions();
        let depth = image.get_depth();

        #[cfg(not(feature = "threads"))]
        {
            trace!("Running gaussian blur in single threaded mode");

            match depth.bit_type()
            {
                BitType::U8 =>
                {
                    let mut temp = vec![0; width * height];

                    for channel in image.get_channels_mut(false)
                    {
                        gaussian_blur_u8(
                            channel.reinterpret_as_mut::<u8>().unwrap(),
                            &mut temp,
                            width,
                            height,
                            self.sigma
                        );
                    }
                }
                BitType::U16 =>
                {
                    let mut temp = vec![0; width * height];

                    for channel in image.get_channels_mut(false)
                    {
                        gaussian_blur_u16(
                            channel.reinterpret_as_mut::<u16>().unwrap(),
                            &mut temp,
                            width,
                            height,
                            self.sigma
                        );
                    }
                }
                _ => todo!()
            }
        }

        #[cfg(feature = "threads")]
        {
            trace!("Running gaussian blur in multithreaded mode");
            std::thread::scope(|s| {
                // blur each channel on a separate thread
                for channel in image.get_channels_mut(false)
                {
                    s.spawn(|| match depth.bit_type()
                    {
                        BitType::U8 =>
                        {
                            let mut temp = vec![0; width * height];

                            gaussian_blur_u8(
                                channel.reinterpret_as_mut::<u8>().unwrap(),
                                &mut temp,
                                width,
                                height,
                                self.sigma
                            );
                        }
                        BitType::U16 =>
                        {
                            let mut temp = vec![0; width * height];

                            gaussian_blur_u16(
                                channel.reinterpret_as_mut::<u16>().unwrap(),
                                &mut temp,
                                width,
                                height,
                                self.sigma
                            );
                        }
                        _ => todo!()
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
