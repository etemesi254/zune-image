#![cfg_attr(feature = "docs", doc(cfg(feature = "heic")))]
#![cfg(feature = "heic")]

//! HEIC decoding support
//!
//! Decoding is done by the delegate library [zune-heic](zune_heic)
//!
//!

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReaderTrait;
use zune_core::colorspace::ColorSpace;
pub use zune_heic::*;

use crate::codecs::ImageFormat;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::metadata::ImageMetadata;
use crate::traits::DecoderTrait;

impl<T> DecoderTrait for HeifDecoder<T>
where
    T: ZByteReaderTrait
{
    fn decode(&mut self) -> Result<Image, ImageErrors> {
        let mut metadata = self.read_headers()?.unwrap();

        let pixels = self.decode()?;
        let w = self.width().unwrap();
        let h = self.height().unwrap();
        let colorspace = self.colorspace().unwrap();

        let mut image = Image::from_u8(&pixels, w, h, colorspace);

        #[cfg(feature = "metadata")]
        {
            use exif::{Tag, Value};
            // apple heic can have rotated params, which usually stated in
            // the nested  itemproperty::irot and also in exif,
            // the decoder rotates the image to keep it that the decoder produces
            // what one sees but if there is exif data, it is not modified
            // so the pipeline is
            // decode->rotate->encode
            // but on encoding, the exif rotate still points it at whatever the rotated
            // value was meaning that it looks wrong, so to fix we need to indicate in the
            // exif that this image is not rotated, so doing that here
            if let Some(exif) = &mut metadata.exif {
                for field in exif {
                    // set orientation to do nothing
                    if field.tag == Tag::Orientation {
                        field.value = Value::Byte(vec![1]);
                    }
                }
            }
        }
        image.metadata = metadata;

        Ok(image)
    }
    fn dimensions(&self) -> Option<(usize, usize)> {
        Some((self.width().unwrap(), self.height().unwrap()))
    }
    fn out_colorspace(&self) -> ColorSpace {
        self.colorspace().unwrap()
    }
    fn name(&self) -> &'static str {
        "HEIF Decoder"
    }
    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, ImageErrors> {
        self.decode_headers()?;
        let (w, h) = self.dimensions().unwrap();
        let depth = BitDepth::Eight;

        let mut metadata = ImageMetadata {
            format: Some(ImageFormat::HEIC),
            colorspace: self.colorspace().expect("Impossible"),
            depth: depth,
            width: w,
            height: h,
            ..Default::default()
        };
        #[cfg(feature = "metadata")]
        {
            // see if we have an exif chunk
            if let Some(exif) = self.exif_data() {
                metadata.parse_raw_exif(exif)
            }
        }
        if let Some(icc) = self.icc_data() {
            metadata.set_icc_chunk(icc.to_vec());
        }
        Ok(Some(metadata))
    }
}

impl From<HeicErrors> for ImageErrors {
    fn from(value: HeicErrors) -> Self {
        Self::ImageDecodeErrors(format!("heif: {:?}", value))
    }
}
