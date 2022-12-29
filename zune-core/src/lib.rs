pub mod bit_depth;
pub mod bytestream;
pub mod colorspace;

/// A simple enum that can hold either
/// u8's or u16's used for decoding pixels
pub enum DecodingResult
{
    U8(Vec<u8>),
    U16(Vec<u16>)
}

impl DecodingResult
{
    /// Return the contents if the enum stores Vec<u8> or otherwise
    /// return None.
    ///
    /// Useful for de-sugaring the result of a decoding operation
    /// into raw bytes
    ///
    /// # Example
    /// ```
    /// use zune_core::DecodingResult;
    /// let data = DecodingResult::U8(vec![0;100]);
    /// // we know this won't fail because we created it with u8
    /// assert!(data.u8().is_some());
    ///
    /// let data = DecodingResult::U16(vec![0;100]);
    /// // it should now return nothing since the type is u18
    /// assert!(data.u8().is_none());
    ///
    /// ```
    pub fn u8(self) -> Option<Vec<u8>>
    {
        match self
        {
            DecodingResult::U8(data) => Some(data),
            _ => None
        }
    }
}
