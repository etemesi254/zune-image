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
