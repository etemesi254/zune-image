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
    pub const fn num_components(&self) -> usize
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

    pub const fn has_alpha(&self) -> bool
    {
        matches!(self, Self::RGBA | Self::LumaA)
    }

    pub const fn is_grayscale(&self) -> bool
    {
        matches!(self, Self::LumaA | Self::Luma)
    }
}

/// Encapsulates all colorspaces supported by
/// the library
pub static ALL_COLORSPACES: [ColorSpace; 7] = [
    ColorSpace::RGB,
    ColorSpace::RGBA,
    ColorSpace::RGBX,
    ColorSpace::LumaA,
    ColorSpace::Luma,
    ColorSpace::CYMK,
    ColorSpace::YCbCr,
];
