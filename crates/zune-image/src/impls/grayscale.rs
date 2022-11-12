use log::warn;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::grayscale::rgb_to_grayscale;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Convert RGB data to grayscale
///
/// This will convert any image that contains three
/// RGB channels(including RGB, RGBA,RGBX) into grayscale
///
/// Formula for RGB to grayscale conversion is given by
///
/// ```text
///Grayscale = 0.299R + 0.587G + 0.114B
/// ```
/// but it's implemented using fixed point integer mathematics and simd kernels
/// where applicable (see zune-imageprocs/grayscale)
pub struct RgbToGrayScale
{
    preserve_alpha: bool,
}

impl RgbToGrayScale
{
    #[allow(clippy::new_without_default)]
    pub fn new() -> RgbToGrayScale
    {
        RgbToGrayScale {
            preserve_alpha: false,
        }
    }
    pub fn preserve_alpha(mut self, yes: bool) -> RgbToGrayScale
    {
        self.preserve_alpha = yes;
        self
    }
}
impl OperationsTrait for RgbToGrayScale
{
    fn get_name(&self) -> &'static str
    {
        "RGB to Grayscale"
    }

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let im_colorspace = image.get_colorspace();

        if im_colorspace == ColorSpace::Luma || im_colorspace == ColorSpace::LumaA
        {
            warn!("Image already in grayscale skipping this operation");
            return Ok(());
        }

        let (width, height) = image.get_dimensions();
        let size = width * height;

        let mut out = vec![0; size];

        let channel = image.get_channels_ref(self.preserve_alpha);

        let r = &channel[0];
        let g = &channel[1];
        let b = &channel[2];

        rgb_to_grayscale(r, g, b, &mut out, image.get_depth().max_value());

        if self.preserve_alpha && image.get_colorspace().has_alpha()
        {
            image.set_channels(vec![out, channel[3].to_vec()]);
            image.set_colorspace(ColorSpace::LumaA);
        }
        else
        {
            image.set_channels(vec![out]);
            image.set_colorspace(ColorSpace::Luma);
        }

        Ok(())
    }

    fn supported_colorspaces(&self) -> &'static [ColorSpace]
    {
        &[
            ColorSpace::RGBA,
            ColorSpace::RGB,
            ColorSpace::LumaA,
            ColorSpace::Luma,
            ColorSpace::RGBX,
        ]
    }
}
