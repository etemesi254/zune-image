use zune_jpeg::ColorSpace;

#[allow(clippy::upper_case_acronyms)]
pub enum ImageColorspace
{
    RGB,
    RGBA,
    YcbCr,
    GrayScale,
    RGBX,
    YCCK,
    CYMK,
}

impl From<zune_jpeg::ColorSpace> for ImageColorspace
{
    fn from(colorspace: ColorSpace) -> Self
    {
        match colorspace
        {
            ColorSpace::GRAYSCALE => ImageColorspace::GrayScale,
            ColorSpace::RGB => ImageColorspace::RGB,
            ColorSpace::RGBA => ImageColorspace::RGBA,
            ColorSpace::RGBX => ImageColorspace::RGBX,
            ColorSpace::YCbCr => ImageColorspace::YcbCr,
            ColorSpace::CMYK => ImageColorspace::CYMK,
            ColorSpace::YCCK => ImageColorspace::YCCK,
        }
    }
}
