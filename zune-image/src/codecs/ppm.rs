use std::fs::File;
use std::io::BufWriter;

use log::info;
use zune_core::colorspace::ColorSpace;
use zune_ppm::PPMEncoder;

use crate::errors::ImgEncodeErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::EncoderTrait;

pub struct SPPMEncoder
{
    colorspace: ColorSpace,
    file:       BufWriter<File>,
}
impl SPPMEncoder
{
    pub fn new(file: BufWriter<File>) -> SPPMEncoder
    {
        Self {
            colorspace: ColorSpace::RGB,
            file,
        }
    }
}

impl EncoderTrait for SPPMEncoder
{
    fn get_name(&self) -> &'static str
    {
        "PPM Encoder"
    }
    fn set_colorspace(&mut self, colorspace: ColorSpace)
    {
        self.colorspace = colorspace;
    }

    fn encode_to_file(&mut self, image: &Image) -> Result<(), ImgEncodeErrors>
    {
        let (width, height) = image.get_dimensions();
        let writer = &mut self.file;
        let mut ppm_encoder = PPMEncoder::new(width, height, writer);
        ppm_encoder.set_colorspace(self.colorspace);

        if image.get_colorspace() == ColorSpace::RGB
        {
            if let ImageChannels::ThreeChannels(data) = image.get_channel_ref()
            {
                info!("Encoding the image as RGB");
                ppm_encoder.write_rgb((&data[0], &data[1], &data[2]));
                Ok(())
            }
            else if let ImageChannels::Interleaved(data) = image.get_channel_ref()
            {
                info!("Encoding the image as RGB");
                ppm_encoder.write_rgb_interleaved(data);
                Ok(())
            }
            else
            {
                Err(ImgEncodeErrors::GenericStatic(
                    "Fatal error, image colorspace is RGB but RGB buffer is empty",
                ))
            }
        }
        else if image.get_colorspace() == ColorSpace::Luma
        {
            return if let ImageChannels::OneChannel(data) = image.get_channel_ref()
            {
                info!("Encoding the image as grayscale ppm");
                ppm_encoder.write_grayscale(data);
                Ok(())
            }
            else
            {
                Err(ImgEncodeErrors::GenericStatic(
                    "Fatal error, image colorspace is grayscale but grayscale buffer is empty",
                ))
            };
        }
        else
        {
            Err(ImgEncodeErrors::Generic(format!(
                "Fatal error, image colorspace {:?} not supported by PPM format",
                image.get_colorspace()
            )))
        }
    }
}
