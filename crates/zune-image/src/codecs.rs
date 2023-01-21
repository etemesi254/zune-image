//! Entry point for all supported codecs  
//! the library contains
//!
//! Current status
//!
//! |IMAGE    | Decoder      |Encoder|
//! |---------|--------------|-------|
//! |JPEG     |Full support  | None |
//! |PNG      |Partial       |None |
//! |PPM      | 8 and 16 bit support |8 and 16 bit support|
//! |PAL      | None |8 and 16 bit support |
//! | Farbfeld|16 bit support|None|
//!
//!
use zune_core::options::DecoderOptions;

use crate::codecs::ppm::PPMEncoder;
use crate::codecs::qoi::QoiEncoder;
#[allow(unused_imports)]
use crate::traits::DecoderTrait;
use crate::traits::EncoderTrait;

pub mod farbfeld;
pub mod jpeg;
pub mod png;
pub mod ppm;
pub mod psd;
pub mod qoi;
/// All supported decoders
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SupportedDecoders
{
    /// Fully complete
    Jpeg,
    /// Not yet complete
    Png,
    /// Fully complete
    PPM,
    /// Partial support
    PSD,
    /// Full support
    Farbfeld,
    /// Full support
    QOI,
    /// Any unknown format.
    Unknown
}

/// All supported encoders
#[derive(Debug, Copy, Clone)]
pub enum SupportedEncoders
{
    PPM,
    QOI
}

// stolen from imagers
static MAGIC_BYTES: [(&[u8], SupportedDecoders); 8] = [
    (&[137, 80, 78, 71, 13, 10, 26, 10], SupportedDecoders::Png),
    // Of course with jpg we need to relax our definition of what is a jpeg
    // the best identifier would be 0xFF,0xd8 0xff but nop, some images exist
    // which do not have that
    (&[0xff, 0xd8], SupportedDecoders::Jpeg),
    (b"P5", SupportedDecoders::PPM),
    (b"P6", SupportedDecoders::PPM),
    (b"P7", SupportedDecoders::PPM),
    (b"8BPS", SupportedDecoders::PSD),
    (b"farbfeld", SupportedDecoders::Farbfeld),
    (b"qoif", SupportedDecoders::QOI)
];
/// Return the format of an image or none if it's unsupported
pub fn guess_format(bytes: &[u8]) -> Option<SupportedDecoders>
{
    for (magic, decoder) in MAGIC_BYTES
    {
        if bytes.starts_with(magic)
        {
            return Some(decoder);
        }
    }
    None
}

/// Get a decoder capable of decoding `codec` bytes represented by `data`
///
/// This does not handle special form decoders, i.e it uses default settings
/// for decoders
pub fn get_decoder<'a>(codec: SupportedDecoders, data: &'a [u8]) -> Box<dyn DecoderTrait + 'a>
{
    get_decoder_with_options(codec, data, DecoderOptions::default())
}

pub fn get_decoder_with_options<'a>(
    codec: SupportedDecoders, data: &'a [u8], options: DecoderOptions
) -> Box<dyn DecoderTrait + 'a>
{
    match codec
    {
        SupportedDecoders::Jpeg =>
        {
            #[cfg(feature = "jpeg")]
            {
                Box::new(zune_jpeg::JpegDecoder::new_with_options(options, data))
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
                Box::new(zune_png::PngDecoder::new_with_options(data, options))
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
                Box::new(zune_ppm::PPMDecoder::new_with_options(options, data))
            }
            #[cfg(not(feature = "ppm"))]
            {
                unimplemented!("PPM feature not included")
            }
        }
        SupportedDecoders::PSD =>
        {
            #[cfg(feature = "ppm")]
            {
                Box::new(zune_psd::PSDDecoder::new(data))
            }
            #[cfg(not(feature = "ppm"))]
            {
                unimplemented!("PPM feature not included")
            }
        }

        SupportedDecoders::Farbfeld =>
        {
            #[cfg(feature = "farbfeld")]
            {
                Box::new(zune_farbfeld::FarbFeldDecoder::new(data))
            }
            #[cfg(not(feature = "farbfeld"))]
            {
                unimplemented!("Farbfeld feature not included")
            }
        }

        SupportedDecoders::QOI =>
        {
            #[cfg(feature = "qoi")]
            {
                Box::new(zune_qoi::QoiDecoder::new(data))
            }
            #[cfg(not(feature = "qoi"))]
            {
                unimplemented!("QOI feature not included")
            }
        }
        SupportedDecoders::Unknown =>
        {
            panic!("Unknown format encountered")
        }
    }
}

/// Get encoder that can encode an image to a specific extension
pub fn get_encoder_for_extension<P: AsRef<str>>(
    extension: P
) -> Option<(SupportedEncoders, Box<dyn EncoderTrait>)>
{
    match extension.as_ref()
    {
        "qoi" => Some((SupportedEncoders::QOI, Box::new(QoiEncoder::new()))),
        "ppm" | "pam" | "pgm" | "pbm" =>
        {
            Some((SupportedEncoders::PPM, Box::new(PPMEncoder::new())))
        }
        _ => None
    }
}
