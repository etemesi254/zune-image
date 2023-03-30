use zune_core::bit_depth::BitType;
use zune_imageprocs::flip::flip;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct Flip;

impl Flip
{
    pub fn new() -> Flip
    {
        Self::default()
    }
}
impl OperationsTrait for Flip
{
    fn get_name(&self) -> &'static str
    {
        "Flip"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let depth = image.get_depth();

        for inp in image.get_channels_mut(false)
        {
            match depth.bit_type()
            {
                BitType::U8 =>
                {
                    flip(inp.reinterpret_as_mut::<u8>().unwrap());
                }
                BitType::U16 =>
                {
                    flip(inp.reinterpret_as_mut::<u16>().unwrap());
                }
                _ => todo!()
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8, BitType::U16]
    }
}
