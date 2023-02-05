use zune_core::bit_depth::BitType;
use zune_imageprocs::transpose::{transpose_u16, transpose_u8};

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Transpose an image
///
/// This mirrors the image along the image top left to bottom-right
/// diagonal
///
/// Done by swapping X and Y indices of the array representation
#[derive(Default)]
pub struct Transpose;

impl Transpose
{
    pub fn new() -> Transpose
    {
        Transpose::default()
    }
}
impl OperationsTrait for Transpose
{
    fn get_name(&self) -> &'static str
    {
        "Transpose"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();
        let out_dim = width * height * image.get_depth().size_of();

        let depth = image.get_depth();

        for channel in image.get_channels_mut(false)
        {
            let mut out_channel = Channel::new_with_length(out_dim);

            match depth.bit_type()
            {
                BitType::Eight =>
                {
                    transpose_u8(
                        channel.reinterpret_as::<u8>().unwrap(),
                        out_channel.reinterpret_as_mut::<u8>().unwrap(),
                        width,
                        height
                    );
                    *channel = out_channel;
                }
                BitType::Sixteen =>
                {
                    transpose_u16(
                        channel.reinterpret_as::<u16>().unwrap(),
                        out_channel.reinterpret_as_mut::<u16>().unwrap(),
                        width,
                        height
                    );
                    *channel = out_channel;
                }
                _ => todo!()
            };
        }

        image.set_dimensions(height, width);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::Eight, BitType::Sixteen]
    }
}
