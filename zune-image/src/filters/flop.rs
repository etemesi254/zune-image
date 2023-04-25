use zune_core::bit_depth::BitType;
use zune_imageprocs::flop::flop;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct Flop;

impl Flop
{
    pub fn new() -> Flop
    {
        Self::default()
    }
}
impl OperationsTrait for Flop
{
    fn get_name(&self) -> &'static str
    {
        "Flop"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let (width, _) = image.get_dimensions();
        let depth = image.get_depth();

        for channel in image.get_channels_mut(false)
        {
            match depth.bit_type()
            {
                BitType::U8 =>
                {
                    flop(channel.reinterpret_as_mut::<u8>().unwrap(), width);
                }
                BitType::U16 =>
                {
                    flop(channel.reinterpret_as_mut::<u16>().unwrap(), width);
                }
                BitType::F32 =>
                {
                    flop(channel.reinterpret_as_mut::<f32>().unwrap(), width);
                }
                _ => todo!()
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
