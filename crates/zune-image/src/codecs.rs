//! Entry point for all supported codecs  
//! the library contains
//!
//! Current status
//!
//! |IMAGE| Decoder     |Encoder|
//! |-----|-------------|-------|
//! |JPEG |Full support| None |
//! |PNG |Partial  |None |
//! |PPM | 8 and 16 bit support |8 and 16 bit support|
//! |PAL | None |8 and 16 bit support |
//!
//!
#[allow(unused_imports)]
use crate::traits::DecoderTrait;

pub mod jpeg;
pub mod png;
pub mod ppm;

/// All supported decoders
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SupportedDecoders
{
    /// Fully complete
    Jpeg,
    /// Not yet complete
    Png,
    /// Fully complete
    PPM
}

/// All supported encoders
#[derive(Debug)]
pub enum SupportedEncoders
{
    PPM,
    //PPM encoder
    PAM // PAM encoder
}

/// Return the format of an image or none if it's unsupported
pub fn guess_format(bytes: &[u8]) -> Option<SupportedDecoders>
{
    if let Some(magic) = bytes.get(0..2)
    {
        if magic == (0xffd8_u16).to_be_bytes()
        {
            // jpeg bits
            return Some(SupportedDecoders::Jpeg);
        }
    }
    if let Some(magic) = bytes.get(0..8)
    {
        if magic == [137, 80, 78, 71, 13, 10, 26, 10]
        {
            // png signature
            return Some(SupportedDecoders::Png);
        }
    }
    if let Some(bytes) = bytes.get(0..2)
    {
        if bytes[0] == b'P' && matches!(bytes[1], b'5' | b'6')
        {
            return Some(SupportedDecoders::PPM);
        }
    }
    None
}

/// Get a decoder capable of decoding `codec` bytes represented by `data`
///
/// This does not handle special form decoders, i.e it uses default settings
/// for decoders
#[cfg(any(feature = "png", feature = "jpeg"))]
pub fn get_decoder<'a>(codec: SupportedDecoders, data: &'a [u8]) -> Box<dyn DecoderTrait + 'a>
{
    match codec
    {
        SupportedDecoders::Jpeg =>
        {
            #[cfg(feature = "jpeg")]
            {
                Box::new(zune_jpeg::JpegDecoder::new(data))
            }
            #[cfg(not(feature = "jpeg"))]
            {
                unimplemented!("JPEG feature not included")
            }
        }

        SupportedDecoders::Png =>
        {
            #[cfg(feature = "png")]
            {
                Box::new(zune_png::PngDecoder::new(data))
            }
            #[cfg(not(feature = "png"))]
            {
                unimplemented!("PNG feature not included")
            }
        }
        SupportedDecoders::PPM =>
        {
            #[cfg(feature = "ppm")]
            {
                Box::new(zune_ppm::PPMDecoder::new(data))
            }
            #[cfg(not(feature = "ppm"))]
            {
                unimplemented!("PPM feature not included")
            }
        }
    }
}
