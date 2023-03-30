use log::trace;
use zune_core::bit_depth::BitType;
use zune_imageprocs::gamma::gamma;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Gamma adjust an image
#[derive(Default)]
pub struct Gamma
{
    value: f32
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let max_value = image.get_depth().max_value();

        let depth = image.get_depth();
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running gamma correction in single threaded mode");

            for channel in image.get_channels_mut(false)
            {
                match depth.bit_type()
                {
                    BitType::U16 => gamma(
                        channel.reinterpret_as_mut::<u16>().unwrap(),
                        self.value,
                        max_value
                    ),
                    BitType::U8 => gamma(
                        channel.reinterpret_as_mut::<u8>().unwrap(),
                        self.value,
                        max_value
                    ),
                    _ => todo!()
                }
            }
        }
        #[cfg(feature = "threads")]
        {
            trace!("Running gamma correction in multithreaded mode");

            std::thread::scope(|s| {
                for channel in image.get_channels_mut(false)
                {
                    s.spawn(|| match depth.bit_type()
                    {
                        BitType::U16 => gamma(
                            channel.reinterpret_as_mut::<u16>().unwrap(),
                            self.value,
                            max_value
                        ),
                        BitType::U8 => gamma(
                            channel.reinterpret_as_mut::<u8>().unwrap(),
                            self.value,
                            max_value
                        ),
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
