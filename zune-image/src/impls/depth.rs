use log::trace;
use zune_core::bit_depth::{BitDepth, BitType};
use zune_imageprocs::depth::{depth_u16_to_u8, depth_u8_to_u16};

use crate::channel::Channel;
use crate::errors::ImageErrors;
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
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
                (BitDepth::Eight, BitDepth::Sixteen) =>
                {
                    let old_data = channel.reinterpret_as().unwrap();
                    let mut new_channel = Channel::new_with_length::<u16>(old_data.len() * 2);

                    let new_channel_raw = new_channel.reinterpret_as_mut().unwrap();

                    depth_u8_to_u16(old_data, new_channel_raw, self.depth.max_value());

                    *channel = new_channel;
                }

                (BitDepth::Sixteen, BitDepth::Eight) =>
                {
                    let old_data = channel.reinterpret_as::<u16>().unwrap();
                    let mut new_channel = Channel::new_with_length::<u8>(channel.len() / 2);

                    let new_channel_raw = new_channel.reinterpret_as_mut().unwrap();

                    depth_u16_to_u8(old_data, new_channel_raw, image_depth.max_value());

                    *channel = new_channel;
                }
                (BitDepth::Float32, BitDepth::Eight) =>
                {
                    let old_data = channel.reinterpret_as::<f32>().unwrap();
                    let mut new_channel = Channel::new_with_length::<u8>(channel.len() / 4);

                    let new_channel_raw = new_channel.reinterpret_as_mut::<u8>().unwrap();

                    // scale by multiplying with 255
                    for (old_chan, new_chan) in old_data.iter().zip(new_channel_raw.iter_mut())
                    {
                        *new_chan = (255.0 * old_chan).clamp(0.0, 255.0) as u8;
                    }

                    *channel = new_channel;
                }
                (BitDepth::Float32, BitDepth::Sixteen) =>
                {
                    let old_data = channel.reinterpret_as::<f32>().unwrap();
                    let mut new_channel = Channel::new_with_length::<u16>(channel.len() / 2);

                    let new_channel_raw = new_channel.reinterpret_as_mut::<u16>().unwrap();

                    // scale by multiplying with 65535
                    for (old_chan, new_chan) in old_data.iter().zip(new_channel_raw.iter_mut())
                    {
                        *new_chan = (65535.0 * old_chan).clamp(0.0, 65535.0) as u16;
                    }

                    *channel = new_channel;
                }

                (_, _) =>
                {
                    let msg = format!(
                        "Unknown depth conversion from {:?} to {:?}",
                        image_depth, self.depth
                    );

                    return Err(ImageErrors::GenericString(msg));
                }
            }
        }
        trace!("Image depth changed to {:?}", self.depth);

        image.set_depth(self.depth);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
