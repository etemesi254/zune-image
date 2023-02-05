use zune_core::bit_depth::BitType;
use zune_imageprocs::flop::flop;

use crate::errors::ImgOperationsErrors;
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, _) = image.get_dimensions();
        let depth = image.get_depth();

        for channel in image.get_channels_mut(false)
        {
            match depth.bit_type()
            {
                BitType::Eight =>
                {
                    flop(channel.reinterpret_as_mut::<u8>().unwrap(), width);
                }
                BitType::Sixteen =>
                {
                    flop(channel.reinterpret_as_mut::<u16>().unwrap(), width);
                }
                _ => todo!()
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::Eight, BitType::Sixteen]
    }
}
