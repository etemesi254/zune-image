/// The image bit depth.
///
/// The library successfully supports depths up to
/// 16 bits, as the underlying storage is usually a `u16`.
///
/// This allows us to comfortably support a wide variety of images
/// e.g 10 bit av1, 16 bit png and ppm.
#[derive(Copy, Clone, Debug)]
pub enum BitDepth
{
    // Most common images
    Eight,
    // AV1
    Ten,
    // HDR
    Twelve,
    // PPM/PNM, 16 bit png.
    Sixteen,
}
impl Default for BitDepth
{
    fn default() -> Self
    {
        Self::Eight
    }
}

impl BitDepth
{
    /// Get the max value supported by the bit depth
    ///
    /// During conversion from one bit depth to another
    ///
    /// larger values should be clamped to this bit depth
    #[rustfmt::skip]
    #[allow(clippy::zero_prefixed_literal)]
    pub const fn max_value(self) -> u16
    {
        match self
        {
            Self::Eight   => (1 << 08) - 1,
            Self::Ten     => (1 << 10) - 1,
            Self::Twelve  => (1 << 12) - 1,
            Self::Sixteen => u16::MAX,
        }
    }
}
