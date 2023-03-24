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
    /// U8 bit depth.
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
    /// U16 bit depth
    ///
    /// Images with such bit depths use [`u16`] to store values and use the whole range
    /// i.e 0-65535
    ///
    /// Data is stored and processed in native endian.
    Sixteen,
    /// Bit depth information is unknown
    Unknown
}

/// The underlying bit representation of the image
///
/// This represents the minimum rust type that
/// can be used to represent image data, required
/// by `Channel` struct in zune-image
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BitType
{
    /// Images represented using a [`u8`] as their
    /// underlying pixel storage
    U8,
    /// Images represented using a [`u16`] as their
    /// underlying pixel storage.
    U16
}

impl Default for BitDepth
{
    fn default() -> Self
    {
        Self::Unknown
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
            Self::Sixteen => u16::MAX,
            Self::Unknown => 0,
        }
    }

    /// Return the minimum number of bits that can be used to represent
    /// each pixel in the image
    ///
    /// All bit depths below 8 return a bit type of `BitType::U8`.
    ///  and all those above 8 and below 16 return a bit type of `BitType::SixTeen`
    ///
    /// # Returns
    /// An enum whose variants represent the minimum size for an unsigned integer
    /// which can store the image pixels without overflow
    ///
    /// # Example
    ///
    /// ```
    /// use zune_core::bit_depth::{BitDepth, BitType};
    /// assert_eq!(BitDepth::Eight.bit_type(),BitType::U8);
    ///
    /// assert_eq!(BitDepth::Sixteen.bit_type(),BitType::U16);
    /// ```
    ///
    /// See also [size_of](BitDepth::size_of)
    pub const fn bit_type(self) -> BitType
    {
        match self
        {
            Self::Eight => BitType::U8,
            Self::Sixteen => BitType::U16,
            Self::Unknown => panic!("Unknown bit type")
        }
    }
    /// Get the number of bytes needed to store a specific bit depth
    ///
    ///  
    /// # Example
    /// For images less than or equal to 8 bits(1 byte), we can use a [`u8`] to store
    /// the pixels, and a size_of [`u8`] is 1
    ///
    /// For images greater than 8  bits and less than 16 bits(2 bytes), we can use a [`u16`] to
    /// store the pixels, a size_of [`u16`] is 2.
    /// ```
    /// use zune_core::bit_depth::BitDepth;
    /// let depth = BitDepth::Sixteen;
    /// // greater 12 bits is greater than 8 and less than 16
    /// assert_eq!(depth.size_of(),2);
    /// ```
    pub const fn size_of(self) -> usize
    {
        match self
        {
            Self::Eight => 1,
            Self::Sixteen => 2,
            Self::Unknown => panic!("Unknown bit type")
        }
    }
    pub const fn bit_size(&self) -> usize
    {
        match self
        {
            Self::Eight => 8,
            Self::Sixteen => 16,
            Self::Unknown => panic!("Unknown bit depth")
        }
    }
}
