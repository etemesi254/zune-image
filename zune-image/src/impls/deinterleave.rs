use log::warn;
use zune_imageprocs::deinterleave::{de_interleave_four_channels, de_interleave_three_channels};

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

#[derive(Default)]
pub struct DeInterleaveChannels;

impl DeInterleaveChannels
{
    pub fn new() -> DeInterleaveChannels
    {
        DeInterleaveChannels {}
    }
}
impl OperationsTrait for DeInterleaveChannels
{
    fn get_name(&self) -> &'static str
    {
        "De-interleave channels"
    }

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        let colorspace = image.get_colorspace();

        let channel = image.get_channel_mut();
        if !channel.is_interleaved()
        {
            warn!("Image channels already de-interleaved, skipping this operation");
            return Ok(());
        }

        if let ImageChannels::Interleaved(rgb_channel) = channel
        {
            // three component de-interleave
            if colorspace.num_components() == 3
            {
                // allocate new array based on width and height
                let size = width * height;

                if rgb_channel.len() != size * 3
                {
                    return Err(ImgOperationsErrors::Generic(
                        "Image length mismatch, image dimensions do not match array length",
                    ));
                }

                let mut c1 = vec![0; size];
                let mut c2 = vec![0; size];
                let mut c3 = vec![0; size];

                de_interleave_three_channels(rgb_channel, (&mut c1, &mut c2, &mut c3));

                // change the channel type to be uninitialized rgb8
                *channel = ImageChannels::ThreeChannels([c1, c2, c3]);
            }
            else if colorspace.num_components() == 4
            {
                // four component deinterleave
                // allocate new array based on width and height
                let size = width * height;

                if rgb_channel.len() != size * 4
                {
                    return Err(ImgOperationsErrors::Generic(
                        "Image length mismatch, image dimensions do not match array length",
                    ));
                }

                let mut c1 = vec![0; size];
                let mut c2 = vec![0; size];
                let mut c3 = vec![0; size];
                let mut c4 = vec![0; size];

                de_interleave_four_channels(rgb_channel, (&mut c1, &mut c2, &mut c3, &mut c4));

                // change the channel type to be separated rgba
                *channel = ImageChannels::FourChannels([c1, c2, c3, c4]);
            }
        }

        Ok(())
    }
}
