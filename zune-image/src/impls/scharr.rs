use zune_core::bit_depth::BitType;
use zune_imageprocs::scharr::scharr_int;

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Invert
#[derive(Default, Copy, Clone)]
pub struct Scharr;

impl Scharr
{
    pub fn new() -> Scharr
    {
        Self::default()
    }
}

impl OperationsTrait for Scharr
{
    fn get_name(&self) -> &'static str
    {
        "Scharr"
    }
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let depth = image.get_depth().bit_type();
        let (width, height) = image.get_dimensions();

        #[cfg(not(feature = "threads"))]
        {
            for channel in image.get_channels_mut(true)
            {
                let mut out_channel = Channel::new_with_length(channel.len());
                match depth
                {
                    BitType::U8 => scharr_int::<u8>(
                        channel.reinterpret_as().unwrap(),
                        out_channel.reinterpret_as_mut().unwrap(),
                        width,
                        height
                    ),
                    BitType::U16 => scharr_int::<u16>(
                        channel.reinterpret_as().unwrap(),
                        out_channel.reinterpret_as_mut().unwrap(),
                        width,
                        height
                    ),
                    _ => todo!()
                }
                *channel = out_channel;
            }
        }
        #[cfg(feature = "threads")]
        {
            std::thread::scope(|s| {
                for channel in image.get_channels_mut(true)
                {
                    s.spawn(|| {
                        let mut out_channel = Channel::new_with_length(channel.len());
                        match depth
                        {
                            BitType::U8 => scharr_int::<u8>(
                                channel.reinterpret_as().unwrap(),
                                out_channel.reinterpret_as_mut().unwrap(),
                                width,
                                height
                            ),
                            BitType::U16 => scharr_int::<u16>(
                                channel.reinterpret_as().unwrap(),
                                out_channel.reinterpret_as_mut().unwrap(),
                                width,
                                height
                            ),
                            _ => todo!()
                        }
                        *channel = out_channel;
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
