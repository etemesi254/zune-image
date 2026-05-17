#![cfg(feature = "jpeg_2000")]

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::DecoderTrait;
use hayro_jpeg2000;
use hayro_jpeg2000::DecodeSettings;
use std::num::{NonZero, NonZeroU32};
use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

pub struct Jpeg2000Decoder<T: ZByteReaderTrait> {
    decoder: ZReader<T>,
    options: DecoderOptions,
    width: usize,
    height: usize,
    colorspace: ColorSpace,
}
impl<T: ZByteReaderTrait> Jpeg2000Decoder<T> {
    pub fn new(data: T) -> Self {
        Jpeg2000Decoder::new_with_options(data, DecoderOptions::default())
    }
    pub fn new_with_options(data: T, decoder_options: DecoderOptions) -> Self {
        Jpeg2000Decoder {
            decoder: ZReader::new(data),
            options: decoder_options,
            width: 0,
            height: 0,
            colorspace: ColorSpace::Unknown,
        }
    }
}
impl<T: ZByteReaderTrait> DecoderTrait for Jpeg2000Decoder<T> {
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        const MB: usize = 1 << 20;
        let mut raw_bytes = Vec::with_capacity(10 * MB);
        let data = self.decoder.read_all(&mut raw_bytes)?;

        let mut settings = DecodeSettings::default();
        settings.strict = self.options.strict_mode();

        let actual_decoder = hayro_jpeg2000::Image::new(raw_bytes.as_ref(), &settings)?;

        let bytes = actual_decoder.decode()?;
        let mut icc_bytes = None;

        let color = match actual_decoder.color_space() {
            hayro_jpeg2000::ColorSpace::Gray => ColorSpace::Luma,
            hayro_jpeg2000::ColorSpace::RGB => ColorSpace::RGB,
            hayro_jpeg2000::ColorSpace::CMYK => ColorSpace::CMYK,
            hayro_jpeg2000::ColorSpace::Unknown { num_channels } => {
                ColorSpace::MultiBand(NonZeroU32::new(u32::from(*num_channels)).unwrap())
            }
            hayro_jpeg2000::ColorSpace::Icc {
                profile,
                num_channels,
            } => {
                // todo, #[cae] (this is best effort),  we propagate ICC to the top to be used
                // by any caller/subsequent operations that may make this into something that
                // makes sense
                let colorspace = match num_channels {
                    1 => ColorSpace::Luma,
                    2 => ColorSpace::LumaA,
                    3 => ColorSpace::RGB,
                    4 => ColorSpace::RGBA,
                    _ => ColorSpace::MultiBand(NonZeroU32::new(u32::from(*num_channels)).unwrap()),
                };
                #[allow(unused_assignments)]
                {
                    icc_bytes = Some(profile);
                }

                self.colorspace = colorspace;
                return Err(ImageErrors::ImageDecodeErrors(
                    "Unsupported ICC color type".to_string(),
                ));
            }
        };
        self.colorspace = color;
        self.width = actual_decoder.width() as usize;
        self.height = actual_decoder.height() as usize;

        let mut img = Image::from_u8(&bytes, self.width, self.height, color);

        #[cfg(feature = "metadata")]
        {
            // add icc if present
            if let Some(icc_ref) = icc_bytes {
                img.metadata_mut().set_icc_chunk(icc_ref.to_vec());
            }
        }

        Ok(img)
    }

    fn dimensions(&self) -> Option<(usize, usize)> {
        Some((self.width, self.height))
    }

    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace
    }

    fn name(&self) -> &'static str {
        "jpeg_2000 decoder (hayro-jpeg2000)"
    }
}

impl From<hayro_jpeg2000::error::DecodeError> for ImageErrors {
    fn from(error: hayro_jpeg2000::error::DecodeError) -> Self {
        return ImageErrors::ImageDecodeErrors(format!("{}", error));
    }
}
