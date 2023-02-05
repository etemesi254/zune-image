use log::trace;
use zune_core::bit_depth::{BitDepth, BitType};
use zune_imageprocs::depth::{depth_u16_to_u8, depth_u8_to_u16};

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Change the image's bit depth from it's initial
/// value to the one specified by this operation.
#[derive(Copy, Clone)]
pub struct Depth
{
    depth: BitDepth
}

impl Depth
{
    pub fn new(depth: BitDepth) -> Depth
    {
        Depth { depth }
    }
}

impl OperationsTrait for Depth
{
    fn get_name(&self) -> &'static str
    {
        "Depth"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let image_depth = image.get_depth();

        if image_depth == self.depth
        {
            trace!("Image depth already matches requested, no-op");
            return Ok(());
        }

        for channel in image.get_channels_mut(false)
        {
            match (image_depth, self.depth)
            {
                (BitDepth::Eight, BitDepth::Ten | BitDepth::Twelve | BitDepth::Sixteen) =>
                {
                    let old_data = channel.reinterpret_as().unwrap();
                    let mut new_channel = Channel::new_with_length(old_data.len());

                    let new_channel_raw = new_channel.reinterpret_as_mut().unwrap();

                    depth_u8_to_u16(old_data, new_channel_raw, self.depth.max_value());

                    *channel = new_channel;
                }

                (BitDepth::Sixteen | BitDepth::Twelve | BitDepth::Ten, BitDepth::Eight) =>
                {
                    let old_data = channel.reinterpret_as().unwrap();
                    let mut new_channel = Channel::new_with_length(old_data.len());

                    let new_channel_raw = new_channel.reinterpret_as_mut().unwrap();

                    depth_u16_to_u8(old_data, new_channel_raw, image_depth.max_value());

                    *channel = new_channel;
                }
                (
                    BitDepth::Sixteen | BitDepth::Ten | BitDepth::Twelve,
                    BitDepth::Twelve | BitDepth::Ten | BitDepth::Sixteen
                ) =>
                {
                    // simple rescaling/clamping byte types do not change so we are okay
                    for pix in channel.reinterpret_as_mut::<u16>().unwrap()
                    {
                        *pix = (*pix).clamp(0_u16, self.depth.max_value());
                    }
                }
                (_, _) =>
                {
                    let msg = format!(
                        "Unknown depth conversion from {:?} to {:?}",
                        image_depth, self.depth
                    );

                    return Err(ImgOperationsErrors::GenericString(msg));
                }
            }
        }
        trace!("Image depth changed to {:?}", self.depth);

        image.set_depth(self.depth);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::Eight, BitType::Sixteen]
    }
}
