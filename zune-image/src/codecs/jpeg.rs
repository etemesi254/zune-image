#![cfg(feature = "zune-jpeg")]
use crate::errors::ImgErrors;
use crate::image::{Image, ImageChannels};
use zune_core::colorspace::ColorSpace;
use zune_jpeg::errors::DecodeErrors;
/// Re-expose jpeg crate here
pub use zune_jpeg::*;

use crate::traits::DecoderTrait;

impl<'a> DecoderTrait<'a> for zune_jpeg::JpegDecoder<'a> {
    fn decode(&mut self) -> Result<Image, crate::errors::ImgErrors> {
        let pixel_data = self
            .decode_buffer()
            .map_err(<DecodeErrors as Into<ImgErrors>>::into)?;

        let colorspace = self.get_out_colorspace();

        let pixels = {
            // One channel we don't need to deinterleave
            if colorspace.num_components() == 1 {
                ImageChannels::OneChannel(pixel_data)
            } else {
                ImageChannels::Interleaved(pixel_data)
            }
        };

        let (width, height) = self.get_dimensions().unwrap();

        let mut image = Image::new();

        image.set_dimensions(width, height);
        image.set_image_channel(pixels);
        image.set_colorspace(colorspace);

        Ok(image)
    }

    fn get_dimensions(&self) -> Option<(usize, usize)> {
        let width = usize::from(self.width());
        let height = usize::from(self.height());

        Some((width, height))
    }

    fn get_out_colorspace(&self) -> ColorSpace {
        self.get_output_colorspace()
    }

    fn get_name(&self) -> &'static str {
        "Jpeg decoder"
    }
}
