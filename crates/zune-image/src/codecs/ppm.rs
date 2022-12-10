#![cfg(feature = "ppm")]
//! Represents a PPM and PAL image encoder
use std::io::Write;

use log::debug;
use zune_core::colorspace::ColorSpace;
use zune_core::DecodingResult;
pub use zune_ppm::PPMDecoder;
use zune_ppm::PPMEncoder as PPMEnc;

use crate::deinterleave::{deinterleave_u16, deinterleave_u8};
use crate::errors::{ImgEncodeErrors, ImgErrors};
use crate::image::Image;
use crate::traits::{DecoderTrait, EncoderTrait};

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

        let version = zune_ppm::version_for_colorspace(colorspace).unwrap();

        if image.get_depth().max_value() > 255
        {
            debug!("Encoding PPM as 16 bit image");
            ppm_encoder.encode_u16(width, height, colorspace, version, &image.flatten::<u16>())?;
        }
        else
        {
            debug!("Encoding PPM as 8 bit image");
            ppm_encoder.encode_u8(width, height, colorspace, version, &image.flatten::<u8>())?;
        }

        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGB,  // p7
            ColorSpace::Luma, // p7
            ColorSpace::RGBA, // p7
            ColorSpace::RGBX, // p7
            ColorSpace::LumaA
        ]
    }
}

impl<'a> DecoderTrait<'a> for PPMDecoder<'a>
{
    fn decode(&mut self) -> Result<Image, ImgErrors>
    {
        let pixels = self.decode()?;

        let depth = self.get_bit_depth().unwrap();
        let (width, height) = self.get_dimensions().unwrap();
        let colorspace = self.get_colorspace().unwrap();

        debug!("De-Interleaving image channel");

        let channel = match pixels
        {
            DecodingResult::U8(data) => deinterleave_u8(&data, colorspace).unwrap(),
            DecodingResult::U16(data) => deinterleave_u16(&data, colorspace).unwrap()
        };

        Ok(Image::new(channel, depth, width, height, colorspace))
    }

    fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        self.get_dimensions()
    }

    fn get_out_colorspace(&self) -> ColorSpace
    {
        self.get_colorspace().unwrap_or(ColorSpace::Unknown)
    }

    fn get_name(&self) -> &'static str
    {
        "PPM Decoder"
    }
}
