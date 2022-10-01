#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ColorSpace
{
    RGB,
    RGBA,
    YCbCr,
    GrayScale,
    LumaA,
    RGBX,
    YCCK,
    CYMK,
    Unknown,
}
impl ColorSpace
{
    pub fn num_components(&self) -> usize
    {
        match self
        {
            Self::RGB | Self::YCbCr => 3,
            Self::RGBX | Self::RGBA | Self::YCCK | Self::CYMK => 4,
            Self::GrayScale => 1,
            Self::LumaA => 2,
            Self::Unknown => 0,
        }
    }
}
