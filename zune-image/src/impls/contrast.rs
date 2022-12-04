use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::contrast::contrast_u8;

use crate::errors::ImgOperationsErrors;
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let depth = image.get_depth();

        for channel in image.get_channels_mut(false)
        {
            match depth.bit_type()
            {
                BitType::Eight =>
                {
                    contrast_u8(channel.reinterpret_as_mut::<u8>().unwrap(), self.contrast)
                }
                BitType::Sixteen =>
                {
                    return Err(ImgOperationsErrors::Generic(
                        "Contrast for 16 bit depth is not yet implemented"
                    ));
                }
            }
        }
        Ok(())
    }
    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGBA,
            ColorSpace::RGBX,
            ColorSpace::RGB,
            ColorSpace::LumaA,
            ColorSpace::Luma
        ]
    }
}
