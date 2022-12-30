use std::mem::ManuallyDrop;

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

    /// Return the contents if the enum stores Vec<u16> or otherwise
    /// return None.
    ///
    /// Useful for de-sugaring the result of a decoding operation
    /// into raw bytes
    ///
    /// # Example
    /// ```
    /// use zune_core::DecodingResult;
    /// let data = DecodingResult::U8(vec![0;100]);
    /// // we know this will fail because we created it with u16
    /// assert!(data.u16().is_none());
    ///
    ///
    /// let data = DecodingResult::U16(vec![0;100]);
    /// // it should now return something since the type is u16
    /// assert!(data.u16().is_some());
    ///
    /// ```
    pub fn u16(self) -> Option<Vec<u16>>
    {
        match self
        {
            DecodingResult::U16(data) => Some(data),
            _ => None
        }
    }

    pub fn u16_to_u8(self)
    {
        if let DecodingResult::U16(data) = self
        {
            unsafe {
                let (ptr, length, capacity) = into_raw_parts(data);
                let new_ptr = ptr.cast::<u8>();

                let _new_value = Vec::from_raw_parts(new_ptr, length * 2, capacity * 2);
            }
        }
    }
}

// implement vec into raw parts but use
fn into_raw_parts<T>(vec: Vec<T>) -> (*mut T, usize, usize)
{
    let mut me = ManuallyDrop::new(vec);
    (me.as_mut_ptr(), me.len(), me.capacity())
}
