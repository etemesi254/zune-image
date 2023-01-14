//! Image bit depth, information and manipulations

/// The image bit depth.
///
/// The library successfully supports depths up to
/// 16 bits, as the underlying storage is usually a `u16`.
///
/// This allows us to comfortably support a wide variety of images
/// e.g 10 bit av1, 16 bit png and ppm.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BitDepth
{
    /// Eight bit depth.
    ///
    /// Images with such bit depth use [`u8`] to store
    /// pixels and use the whole range from 0-255.
    ///
    /// It is currently the smallest supported bit depth
    /// by the library.
    ///
    /// For images with bit depths lower than this, they will be scaled
    /// to this bit depth
    Eight,
    /// Ten bit depth
    ///
    /// Images with such bit depth use [`u16`] to store values but they
    /// only use 10 bits, i.e from 0-1024.
    ///
    /// Preserved across image operations
    ///
    /// This is provided as a compatibility to allow decoding and encoding of av1 images
    /// (when fully supported)
    ///
    /// Data is stored and processed in native endian
    Ten,
    /// Twelve bit depth
    ///
    /// Images with such bit depths use [`u16`] to store values but they
    /// only use 12 bits.
    ///
    /// Data is stored and processed in native endian
    Twelve,
    /// Sixteen bit depth
    ///
    /// Images with such bit depths use [`u16`] to store values and use the whole range
    /// i.e 0-65535
    ///
    /// Data is stored and processed in native endian.
    Sixteen
}

/// The underlying bit representation of the image
///
/// This represents the minimum rust type that
/// can be used to represent image data, required
/// by `Channel` struct in zune-image
#[derive(Copy, Clone, Debug)]
pub enum BitType
{
    Eight,
    Sixteen
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
            Self::Eight => (1 << 08) - 1,
            Self::Ten => (1 << 10) - 1,
            Self::Twelve => (1 << 12) - 1,
            Self::Sixteen => u16::MAX,
        }
    }

    /// Return the bit type that can be used to represent
    pub const fn bit_type(self) -> BitType
    {
        match self
        {
            Self::Eight => BitType::Eight,
            Self::Ten | Self::Twelve | Self::Sixteen => BitType::Sixteen
        }
    }

    pub const fn size_of(self) -> usize
    {
        match self
        {
            Self::Eight => 1,
            Self::Ten | Self::Twelve | Self::Sixteen => 2
        }
    }
}
