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
#![allow(unused_imports, unused_variables)]

use zune_core::options::DecoderOptions;

use crate::codecs;
use crate::traits::{DecoderTrait, EncoderTrait};

pub mod farbfeld;
pub mod jpeg;
pub mod png;
pub mod ppm;
pub mod psd;
pub mod qoi;

/// All supported image formats
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImageFormat
{
    /// Fully complete
    JPEG,
    /// Not yet complete
    PNG,
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

impl ImageFormat
{
    /// Return true if an image format has an encoder
    pub const fn has_encoder(self) -> bool
    {
        match self
        {
            Self::PPM =>
            {
                #[cfg(feature = "ppm")]
                {
                    true
                }
                #[cfg(not(feature = "ppm"))]
                {
                    false
                }
            }
            Self::QOI =>
            {
                #[cfg(feature = "qoi")]
                {
                    true
                }
                #[cfg(not(feature = "qoi"))]
                {
                    false
                }
            }
            // all other formats don't have an encoder
            _ => false
        }
    }

    pub fn get_decoder<'a>(&self, data: &'a [u8]) -> Box<dyn DecoderTrait<'a> + 'a>
    {
        self.get_decoder_with_options(data, DecoderOptions::default())
    }

    pub fn get_decoder_with_options<'a>(
        &self, data: &'a [u8], options: DecoderOptions
    ) -> Box<dyn DecoderTrait<'a> + 'a>
    {
        match self
        {
            ImageFormat::JPEG =>
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

            ImageFormat::PNG =>
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
            ImageFormat::PPM =>
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
            ImageFormat::PSD =>
            {
                #[cfg(feature = "ppm")]
                {
                    Box::new(zune_psd::PSDDecoder::new_with_options(data, options))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    unimplemented!("PPM feature not included")
                }
            }

            ImageFormat::Farbfeld =>
            {
                #[cfg(feature = "farbfeld")]
                {
                    Box::new(zune_farbfeld::FarbFeldDecoder::new_with_options(
                        data, options
                    ))
                }
                #[cfg(not(feature = "farbfeld"))]
                {
                    unimplemented!("Farbfeld feature not included")
                }
            }

            ImageFormat::QOI =>
            {
                #[cfg(feature = "qoi")]
                {
                    Box::new(zune_qoi::QoiDecoder::new_with_options(options, data))
                }
                #[cfg(not(feature = "qoi"))]
                {
                    unimplemented!("QOI feature not included")
                }
            }
            ImageFormat::Unknown =>
            {
                panic!("Unknown format encountered")
            }
        }
    }

    pub fn get_encoder(&self) -> Option<Box<dyn EncoderTrait>>
    {
        match self
        {
            Self::PPM =>
            {
                #[cfg(feature = "ppm")]
                {
                    Some(Box::new(crate::codecs::ppm::PPMEncoder::new()))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    None
                }
            }
            Self::QOI =>
            {
                #[cfg(feature = "ppm")]
                {
                    Some(Box::new(crate::codecs::ppm::PPMEncoder::new()))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    None
                }
            }
            Self::JPEG =>
            {
                #[cfg(feature = "jpeg")]
                {
                    Some(Box::new(crate::codecs::jpeg::JpegEncoder::new(80)))
                }
                #[cfg(not(feature = "jpeg"))]
                {
                    None
                }
            }
            // all encoders not implemented default to none
            _ => None
        }
    }
    pub fn guess_format(bytes: &[u8]) -> Option<ImageFormat>
    {
        // stolen from imagers
        static MAGIC_BYTES: [(&[u8], ImageFormat); 8] = [
            (&[137, 80, 78, 71, 13, 10, 26, 10], ImageFormat::PNG),
            // Of course with jpg we need to relax our definition of what is a jpeg
            // the best identifier would be 0xFF,0xd8 0xff but nop, some images exist
            // which do not have that
            (&[0xff, 0xd8], ImageFormat::JPEG),
            (b"P5", ImageFormat::PPM),
            (b"P6", ImageFormat::PPM),
            (b"P7", ImageFormat::PPM),
            (b"8BPS", ImageFormat::PSD),
            (b"farbfeld", ImageFormat::Farbfeld),
            (b"qoif", ImageFormat::QOI)
        ];

        for (magic, decoder) in MAGIC_BYTES
        {
            if bytes.starts_with(magic)
            {
                return Some(decoder);
            }
        }
        None
    }

    pub fn get_encoder_for_extension<P: AsRef<str>>(
        extension: P
    ) -> Option<(ImageFormat, Box<dyn EncoderTrait>)>
    {
        match extension.as_ref()
        {
            "qoi" =>
            {
                #[cfg(feature = "qoi")]
                {
                    Some((
                        ImageFormat::QOI,
                        Box::new(crate::codecs::qoi::QoiEncoder::new())
                    ))
                }
                #[cfg(not(feature = "qoi"))]
                {
                    None
                }
            }
            "ppm" | "pam" | "pgm" | "pbm" =>
            {
                #[cfg(feature = "ppm")]
                {
                    Some((
                        ImageFormat::PPM,
                        Box::new(crate::codecs::ppm::PPMEncoder::new())
                    ))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    None
                }
            }
            "jpeg" | "jpg" =>
            {
                #[cfg(feature = "jpeg")]
                {
                    Some((
                        ImageFormat::JPEG,
                        Box::new(crate::codecs::jpeg::JpegEncoder::new(80))
                    ))
                }
                #[cfg(not(feature = "jpeg"))]
                {
                    None
                }
            }
            _ => None
        }
    }
}
