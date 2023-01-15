#![cfg(feature = "ppm")]
//! Represents a PPM and PAL image encoder
use log::debug;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_core::result::DecodingResult;
pub use zune_ppm::PPMDecoder;
use zune_ppm::{PPMEncodeErrors, PPMEncoder as PPMEnc};

use crate::codecs::SupportedEncoders;
use crate::deinterleave::{deinterleave_u16, deinterleave_u8};
use crate::errors::{ImgEncodeErrors, ImgErrors};
use crate::image::Image;
use crate::traits::{DecoderTrait, EncoderTrait};

#[derive(Copy, Clone, Default)]
pub struct PPMEncoder;

impl PPMEncoder
{
    pub fn new() -> PPMEncoder
    {
        PPMEncoder {}
    }
}

impl EncoderTrait for PPMEncoder
{
    fn get_name(&self) -> &'static str
    {
        "PPM Encoder"
    }

    fn encode_inner(&mut self, image: &Image) -> Result<Vec<u8>, ImgEncodeErrors>
    {
        let (width, height) = image.get_dimensions();
        let colorspace = image.get_colorspace();
        let depth = image.depth;

        let options = EncoderOptions {
            width,
            height,
            colorspace,
            quality: 0,
            depth
        };
        let data = image.to_u8();

        let mut ppm_encoder = PPMEnc::new(&data, options);

        let data = ppm_encoder
            .encode()
            .map_err(<PPMEncodeErrors as Into<ImgEncodeErrors>>::into)?;

        Ok(data)
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGB,  // p7
            ColorSpace::Luma, // p7
            ColorSpace::RGBA, // p7
            ColorSpace::LumaA
        ]
    }

    fn format(&self) -> SupportedEncoders
    {
        SupportedEncoders::PPM
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
