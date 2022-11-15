#![cfg(feature = "ppm")]

use std::io::Write;

use zune_core::colorspace::ColorSpace;
use zune_ppm::{PAMEncoder as PAMEnc, PPMEncoder as PPMEnc};

use crate::errors::ImgEncodeErrors;
use crate::image::Image;
use crate::traits::EncoderTrait;

pub struct PPMEncoder<'a, W: Write>
{
    file: &'a mut W
}

impl<'a, W> PPMEncoder<'a, W>
where
    W: Write
{
    pub fn new(file: &'a mut W) -> PPMEncoder<W>
    {
        Self { file }
    }
}

impl<'a, W> EncoderTrait for PPMEncoder<'a, W>
where
    W: Write
{
    fn get_name(&self) -> &'static str
    {
        "PPM Encoder"
    }

    fn encode_to_file(&mut self, image: &Image) -> Result<(), ImgEncodeErrors>
    {
        let (width, height) = image.get_dimensions();
        let writer = &mut self.file;

        let colorspace = image.get_colorspace();

        let mut ppm_encoder = PPMEnc::new(writer);

        ppm_encoder.encode_ppm(width, height, colorspace, &image.flatten_u8())?;

        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGB,  // p6
            ColorSpace::Luma  // p5
        ]
    }
}

pub struct PAMEncoder<'a, W: Write>
{
    file: &'a mut W
}

impl<'a, W> PAMEncoder<'a, W>
where
    W: Write
{
    pub fn new(file: &'a mut W) -> PAMEncoder<W>
    {
        Self { file }
    }
}

impl<'a, W> EncoderTrait for PAMEncoder<'a, W>
where
    W: Write
{
    fn get_name(&self) -> &'static str
    {
        "PAM Encoder"
    }

    fn encode_to_file(&mut self, image: &Image) -> Result<(), ImgEncodeErrors>
    {
        let (width, height) = image.get_dimensions();
        let writer = &mut self.file;

        let colorspace = image.get_colorspace();

        let mut pam_encoder = PAMEnc::new(writer);

        if image.get_depth().max_value() > 255
        {
            // use larger bit depth
            pam_encoder.encode_pam_u16(width, height, colorspace, &image.flatten())?;
        }
        else
        {
            // use simple format
            pam_encoder.encode_pam(width, height, colorspace, &image.flatten_u8())?;
        }

        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGB,  // p7
            ColorSpace::Luma, // p7
            ColorSpace::RGBA, // p7
            ColorSpace::RGBX  // p7
        ]
    }
}
