use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::contrast::contrast_u8;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

#[derive(Default)]
pub struct Contrast
{
    contrast: f32
}

impl Contrast
{
    pub fn new(contrast: f32) -> Contrast
    {
        Contrast { contrast }
    }
}

impl OperationsTrait for Contrast
{
    fn get_name(&self) -> &'static str
    {
        "contrast"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let depth = image.get_depth();

        for channel in image.get_channels_mut(true)
        {
            match depth.bit_type()
            {
                BitType::U8 =>
                {
                    contrast_u8(channel.reinterpret_as_mut::<u8>().unwrap(), self.contrast)
                }
                BitType::U16 =>
                {
                    return Err(ImageErrors::GenericStr(
                        "Contrast for 16 bit depth is not yet implemented"
                    ));
                }
                _ => todo!()
            }
        }
        Ok(())
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGBA,
            ColorSpace::RGB,
            ColorSpace::LumaA,
            ColorSpace::Luma
        ]
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8]
    }
}
