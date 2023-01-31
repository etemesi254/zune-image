use zune_core::options::DecoderOptions;

use crate::traits::{DecoderTrait, EncoderTrait};

/// All supported image formats
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImageFormat
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
            ImageFormat::Jpeg =>
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

            ImageFormat::Png =>
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
                    Box::new(zune_psd::PSDDecoder::new(data))
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
                    Box::new(zune_farbfeld::FarbFeldDecoder::new(data))
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
                    Box::new(zune_qoi::QoiDecoder::new(data))
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
            // all encoders not implemented default to none
            _ => None
        }
    }
    pub fn guess_format(bytes: &[u8]) -> Option<ImageFormat>
    {
        // stolen from imagers
        static MAGIC_BYTES: [(&[u8], ImageFormat); 8] = [
            (&[137, 80, 78, 71, 13, 10, 26, 10], ImageFormat::Png),
            // Of course with jpg we need to relax our definition of what is a jpeg
            // the best identifier would be 0xFF,0xd8 0xff but nop, some images exist
            // which do not have that
            (&[0xff, 0xd8], ImageFormat::Jpeg),
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
            _ => None
        }
    }
}
