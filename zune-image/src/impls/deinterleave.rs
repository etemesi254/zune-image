use zune_imageprocs::deinterleave::de_interleave_3_channels;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

#[derive(Default)]
pub struct DeInterleaveThreeChannels;

impl DeInterleaveThreeChannels
{
    pub fn new() -> DeInterleaveThreeChannels
    {
        DeInterleaveThreeChannels {}
    }
}
impl OperationsTrait for DeInterleaveThreeChannels
{
    fn get_name(&self) -> &'static str
    {
        "De-interleave 3 channels"
    }

    fn execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        if image.get_colorspace().num_components() != 3
        {
            return Err(ImgOperationsErrors::WrongComponents(
                3,
                image.get_colorspace().num_components(),
            ));
        }
        let channel = image.get_channel_mut();
        if let ImageChannels::Interleaved(rgb_channel) = channel
        {
            // allocate new array based on width and height
            let size = width * height;

            assert_eq!(
                rgb_channel.len(),
                size * 3,
                "Length is not equal to dimensions"
            );

            let mut c1 = vec![0; size];
            let mut c2 = vec![0; size];
            let mut c3 = vec![0; size];

            de_interleave_3_channels(rgb_channel, (&mut c1, &mut c2, &mut c3));

            // change the channel type to be uninitialized rgb8
            *channel = ImageChannels::ThreeChannels([c1, c2, c3]);
        }
        Ok(())
    }
}
