#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ColorSpace
{
    RGB,
    RGBA,
    YCbCr,
    Luma,
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
            Self::Luma => 1,
            Self::LumaA => 2,
            Self::Unknown => 0,
        }
    }
}
