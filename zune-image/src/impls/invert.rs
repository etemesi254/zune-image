use zune_core::colorspace::ColorSpace;
use zune_imageprocs::invert::invert;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Invert
#[derive(Default)]
pub struct Invert;

impl Invert
{
    pub fn new() -> Invert
    {
        Self::default()
    }
}
impl OperationsTrait for Invert
{
    fn get_name(&self) -> &'static str
    {
        "Invert"
    }
    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        for channel in image.get_channels_mut(false)
        {
            invert(channel)
        }

        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGB,
            ColorSpace::RGBA,
            ColorSpace::LumaA,
            ColorSpace::RGBX,
            ColorSpace::Luma,
        ]
    }
}
