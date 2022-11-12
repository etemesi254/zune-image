#![cfg(feature = "ppm")]
use std::fs::File;
use std::io::BufWriter;

use zune_ppm::PPMEncoder;

use crate::errors::ImgEncodeErrors;
use crate::image::Image;
use crate::traits::EncoderTrait;

pub struct SPPMEncoder
{
    file: BufWriter<File>,
}
impl SPPMEncoder
{
    pub fn new(file: BufWriter<File>) -> SPPMEncoder
    {
        Self { file }
    }
}

impl EncoderTrait for SPPMEncoder
{
    fn get_name(&self) -> &'static str
    {
        "PPM Encoder"
    }

    fn encode_to_file(&mut self, image: &Image) -> Result<(), ImgEncodeErrors>
    {
        let (width, height) = image.get_dimensions();
        let writer = &mut self.file;
        let mut ppm_encoder = PPMEncoder::new(width, height, image.get_colorspace(), writer);
        ppm_encoder.write(&image.flatten_u8());
        Ok(())
    }
}
